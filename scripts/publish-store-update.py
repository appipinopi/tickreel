"""Submit an MSIX update using the Microsoft Store submission API.

The first submission must be completed in Partner Center. Credentials are read
only from environment variables, and tokens/SAS URLs are never logged.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parent.parent
API = "https://manage.devcenter.microsoft.com/v1.0/my"
NS = {"p": "http://schemas.microsoft.com/appx/manifest/foundation/windows10"}


def inspect_package(filename, identity):
    with zipfile.ZipFile(filename) as archive:
        manifest = ET.fromstring(archive.read("AppxManifest.xml"))
        package = manifest.find("p:Identity", NS).attrib
        if package["Name"] != identity["identityName"] or package["Publisher"] != identity["publisher"]:
            raise ValueError("MSIX identity does not match packaging/store-identity.json")
        version = package["Version"]
        if not re.fullmatch(r"\d+\.\d+\.\d+\.0", version) or any(int(part) > 65535 for part in version.split('.')):
            raise ValueError("Invalid Store MSIX version")
        app = manifest.find("p:Applications/p:Application", NS)
        if app.attrib.get("Executable") != "Tickreel.exe" or not {"Tickreel.exe", "ffmpeg.exe"}.issubset(archive.namelist()):
            raise ValueError("MSIX is missing the expected executable or FFmpeg")
        if archive.testzip():
            raise ValueError("MSIX archive is corrupt")
        return {"version": version, "architecture": package["ProcessorArchitecture"].lower()}


def updated_packages(existing, package, filename):
    result = copy.deepcopy(existing)
    incoming = tuple(map(int, package["version"].split('.')))
    for old in result:
        # Preserve packages for all other architectures.
        if old.get("architecture", "").lower() == package["architecture"]:
            if tuple(map(int, old["version"].split('.'))) >= incoming:
                raise ValueError("Store updates require a version newer than the published package")
            old["fileStatus"] = "PendingDelete"
    result.append({"fileName": filename, "fileStatus": "PendingUpload", "minimumDirectXVersion": "None", "minimumSystemRam": "None"})
    return result


def request_json(url, *, method="GET", data=None, token=None, form=None):
    headers = {"Accept": "application/json"}
    body = None
    if token:
        headers["Authorization"] = "Bearer " + token
    if data is not None:
        headers["Content-Type"] = "application/json"
        body = json.dumps(data).encode()
    if form is not None:
        headers["Content-Type"] = "application/x-www-form-urlencoded"
        body = urllib.parse.urlencode(form).encode()
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data=body, headers=headers, method=method), timeout=120) as response:
            raw = response.read(4 * 1024 * 1024)
            return json.loads(raw) if raw else {}
    except urllib.error.HTTPError as error:
        # The URL may contain credentials. Do not stringify the exception.
        raise RuntimeError(f"Microsoft Store API returned HTTP {error.code}") from None
    except urllib.error.URLError:
        raise RuntimeError("Could not connect to the Microsoft Store API") from None


def publish(package_path, identity, package, commit):
    names = ["MS_STORE_TENANT_ID", "MS_STORE_CLIENT_ID", "MS_STORE_CLIENT_SECRET"]
    missing = [name for name in names if not os.environ.get(name)]
    if missing:
        raise ValueError("Missing repository secrets: " + ", ".join(missing))
    tenant = os.environ[names[0]]
    if not re.fullmatch(r"[0-9a-fA-F-]{36}", tenant):
        raise ValueError("MS_STORE_TENANT_ID must be the tenant GUID")
    token = request_json(f"https://login.microsoftonline.com/{tenant}/oauth2/token", method="POST", form={
        "grant_type": "client_credentials", "client_id": os.environ[names[1]],
        "client_secret": os.environ[names[2]], "resource": "https://manage.devcenter.microsoft.com",
    })["access_token"]
    app_url = f"{API}/applications/{identity['storeId']}"
    app = request_json(app_url, token=token)
    published = app.get("lastPublishedApplicationSubmission")
    if not published:
        raise ValueError("Complete the first publication in Partner Center before enabling automatic updates")
    if app.get("pendingApplicationSubmission"):
        raise ValueError("A submission already exists. It will not be overwritten or deleted")
    previous = request_json(f"{app_url}/submissions/{published['id']}", token=token)
    packages = updated_packages(previous.get("applicationPackages", []), package, package_path.name)
    submission = request_json(f"{app_url}/submissions", method="POST", token=token)
    submission_id = submission["id"]
    submission_url = f"{app_url}/submissions/{submission_id}"
    print(f"Created Store submission {submission_id}; existing listings are preserved")
    upload_url = submission.pop("fileUploadUrl")
    parsed = urllib.parse.urlsplit(upload_url)
    if parsed.scheme != "https" or not parsed.hostname or not parsed.hostname.endswith(".blob.core.windows.net"):
        raise ValueError("Microsoft returned an unexpected upload endpoint")
    submission["applicationPackages"] = packages
    # Automatic publication after certification, using the existing listing.
    submission["targetPublishMode"] = "Immediate"
    request_json(submission_url, method="PUT", token=token, data=submission)
    with tempfile.TemporaryDirectory(prefix="tickreel-store-") as temporary:
        zip_path = Path(temporary) / "package.zip"
        with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_STORED) as archive:
            archive.write(package_path, package_path.name)
        # MSIX is already compressed. Keep memory bounded for larger packages.
        import http.client
        connection = http.client.HTTPSConnection(parsed.hostname, timeout=300)
        try:
            connection.putrequest("PUT", urllib.parse.urlunsplit(("", "", parsed.path, parsed.query, "")))
            connection.putheader("x-ms-blob-type", "BlockBlob")
            connection.putheader("Content-Type", "application/zip")
            connection.putheader("Content-Length", str(zip_path.stat().st_size))
            connection.endheaders()
            with zip_path.open("rb") as stream:
                while chunk := stream.read(1024 * 1024):
                    connection.send(chunk)
            response = connection.getresponse()
            if response.status != 201:
                raise RuntimeError(f"Store package upload returned HTTP {response.status}")
            response.read(1024)
        finally:
            connection.close()
    if not commit:
        print(f"Package uploaded to draft submission {submission_id}; not submitted for certification")
        return
    request_json(submission_url + "/commit", method="POST", token=token)
    # Commit completion is distinct from later certification/public availability.
    for _ in range(20):
        status = request_json(submission_url + "/status", token=token)
        state = status.get("status", "Unknown")
        print(f"Store submission {submission_id}: {state}")
        if state in {"CommitFailed", "PreProcessingFailed", "CertificationFailed", "ReleaseFailed"}:
            raise RuntimeError(f"Store submission failed; inspect submission {submission_id} in Partner Center")
        if state not in {"CommitStarted", "PendingCommit"}:
            print("Submitted to Microsoft; certification and publication are pending")
            return
        time.sleep(15)
    raise RuntimeError(f"Commit is still pending for submission {submission_id}; check its status before retrying")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--validate-only", action="store_true", help="Validate locally without credentials or network requests")
    parser.add_argument("--commit", action="store_true", help="Submit the uploaded update for certification")
    args = parser.parse_args()
    identity = json.loads((ROOT / "packaging/store-identity.json").read_text(encoding="utf-8"))
    package = inspect_package(args.package, identity)
    print(f"Validated MSIX {package['version']} ({package['architecture']})")
    if not args.validate_only:
        publish(args.package, identity, package, args.commit)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Never expose access tokens, client secrets or signed upload URLs.
        print(str(error) if isinstance(error, (ValueError, RuntimeError)) else "Store submission failed; inspect the submission in Partner Center", file=sys.stderr)
        sys.exit(1)
