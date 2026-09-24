// Temporary helper: verify the baked-in LMS public keys match the private keys
// under the current IMAGE_LMS_TREE_TYPE / IMAGE_LMS_OTS_TYPE.
use caliptra_image_crypto::{generate_lms_pubkey, OpensslHasher};
use caliptra_image_fake_keys::*;
use caliptra_image_types::{ImageLmsPrivKey, ImageLmsPublicKey};

fn check(name: &str, priv_key: &ImageLmsPrivKey, pub_key: &ImageLmsPublicKey) {
    let computed = generate_lms_pubkey::<OpensslHasher>(priv_key).unwrap();
    assert_eq!(&computed, pub_key, "{name} mismatch");
    println!("{name} OK");
}

#[test]
fn keys_match() {
    check("vnd0", &VENDOR_LMS_KEY_0_PRIVATE, &VENDOR_LMS_KEY_0_PUBLIC);
    check("vnd1", &VENDOR_LMS_KEY_1_PRIVATE, &VENDOR_LMS_KEY_1_PUBLIC);
    check("vnd2", &VENDOR_LMS_KEY_2_PRIVATE, &VENDOR_LMS_KEY_2_PUBLIC);
    check("vnd3", &VENDOR_LMS_KEY_3_PRIVATE, &VENDOR_LMS_KEY_3_PUBLIC);
    check("own", &OWNER_LMS_KEY_PRIVATE, &OWNER_LMS_KEY_PUBLIC);
}
