use app_signature::Signature;
use std::sync::OnceLock;

pub fn signature() -> &'static Signature {
    static SIGNATURE: OnceLock<Signature> = OnceLock::new();
    SIGNATURE.get_or_init(|| {
        Signature::from_json(include_str!("../assets/app-signature.json"))
            .expect("embedded app signature is valid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_links_are_configured_and_arbitrary_targets_are_rejected() {
        assert_eq!(
            signature().resolve("website").unwrap(),
            "https://tickreel.ycccompany.jp/"
        );
        assert_eq!(
            signature().resolve("creator").unwrap(),
            "https://github.com/appipinopi"
        );
        assert!(
            signature()
                .resolve("file:///C:/Windows/system32/cmd.exe")
                .is_err()
        );
    }
}
