import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


store = load('store_publisher', 'publish-store-update.py')
snap = load('snap_preparer', 'prepare-snap-release.py')


class PublishingTests(unittest.TestCase):
    def test_store_preserves_other_architectures(self):
        old = [dict(architecture='X64', version='1.0.0.0', fileStatus='Uploaded'), dict(architecture='ARM64', version='1.0.0.0', fileStatus='Uploaded')]
        original = copy.deepcopy(old)
        updated = store.updated_packages(old, dict(architecture='x64', version='1.0.1.0'), 'update.msix')
        self.assertEqual(updated[0]['fileStatus'], 'PendingDelete')
        self.assertEqual(updated[1]['fileStatus'], 'Uploaded')
        self.assertEqual(updated[-1]['fileStatus'], 'PendingUpload')
        self.assertEqual(old, original)

    def test_store_rejects_same_version_or_downgrade(self):
        old = [dict(architecture='X64', version='1.0.1.0', fileStatus='Uploaded')]
        for version in ['1.0.0.0', '1.0.1.0']:
            with self.assertRaises(ValueError):
                store.updated_packages(old, dict(architecture='x64', version=version), 'update.msix')

    def test_snap_updates_exactly_one_field(self):
        recipe = "version: '1.0.0'\nparts:\n  app:\n    source: https://example.com/old\n"
        self.assertIn("version: '1.0.1'", snap.replace_once(recipe, 'version', "'1.0.1'"))
        self.assertIn('    source: https://example.com/new', snap.replace_once(recipe, 'source', 'https://example.com/new'))
        with self.assertRaises(ValueError):
            snap.replace_once(recipe + '\nsource: duplicate\n', 'source', 'new')

    def test_snap_preserves_desktop_source(self):
        recipe = (ROOT.parent / 'snap/snapcraft.yaml').read_text(encoding='utf-8')
        result = snap.replace_once(recipe, 'source', 'https://example.com/new.tar.gz', r'https://github\.com/appipinopi/tickreel/releases/download/\S+')
        self.assertIn('source: snap/gui', result)
        self.assertIn('source: https://example.com/new.tar.gz', result)


if __name__ == '__main__':
    unittest.main()
