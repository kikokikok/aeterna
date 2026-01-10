#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_hash_consistency(content in "\\PC*") {
            let hash1 = utils::compute_content_hash(&content);
            let hash2 = utils::compute_content_hash(&content);
            prop_assert_eq!(hash1, hash2);
        }

        #[test]
        fn test_hash_different_for_different_content(c1 in "\\PC*", c2 in "\\PC*") {
            if c1 != c2 {
                let hash1 = utils::compute_content_hash(&c1);
                let hash2 = utils::compute_content_hash(&c2);
                prop_assert_ne!(hash1, hash2);
            }
        }
    }
}
