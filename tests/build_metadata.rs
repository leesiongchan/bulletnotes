#[test]
fn compiler_metadata_is_embedded_and_matches_validation_toolchain() {
    let compiler = env!("BULLETNOTES_BUILD_RUSTC");
    assert!(compiler.starts_with("rustc "));
    if let Ok(expected) = std::env::var("BULLETNOTES_EXPECT_BUILD_RUSTC") {
        assert_eq!(compiler, expected);
    }
    println!("embedded_build_compiler: {compiler}");
}
