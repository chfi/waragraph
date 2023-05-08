pub fn path_name_hash_color(path_name: &str) -> [f32; 3] {
    use sha256::digest;
    let hashed = digest(path_name);
    let hashed = hashed.as_bytes();

    let path_r = hashed[24];
    let path_g = hashed[8];
    let path_b = hashed[16];

    let r_f = (path_r as f32) / std::u8::MAX as f32;
    let g_f = (path_g as f32) / std::u8::MAX as f32;
    let b_f = (path_b as f32) / std::u8::MAX as f32;

    let sum = r_f + g_f + b_f;

    [r_f / sum, g_f / sum, b_f / sum]
}
