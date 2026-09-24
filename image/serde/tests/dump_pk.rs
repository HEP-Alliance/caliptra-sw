use caliptra_image_types::*;
use zerocopy::FromBytes;

#[test]
fn dump_vendor_ecc_pubkey() {
    let path = std::env::var("IMG").unwrap();
    let blob = std::fs::read(&path).unwrap();
    let (manifest, _) = ImageManifest::read_from_prefix(&blob[..]).unwrap();
    println!("marker      = 0x{:08x}", manifest.marker);
    println!("size        = {}", manifest.size);
    println!("pqc_key_type= {}", manifest.pqc_key_type);
    let pre = &manifest.preamble;
    println!("ecc_idx     = {}", pre.vendor_ecc_pub_key_idx);
    println!("pqc_idx     = {}", pre.vendor_pqc_pub_key_idx);
    let k = &pre.vendor_ecc_active_pub_key;
    let hex = |w: &[u32]| w.iter().map(|v| format!("{v:08x}")).collect::<Vec<_>>().join("");
    println!("VND_X {}", hex(&k.x));
    println!("VND_Y {}", hex(&k.y));
    let s = &pre.vendor_sigs.ecc_sig;
    println!("VND_R {}", hex(&s.r));
    println!("VND_S {}", hex(&s.s));
    let o = &pre.owner_pub_keys.ecc_pub_key;
    println!("OWN_X {}", hex(&o.x));
    println!("OWN_Y {}", hex(&o.y));
}
