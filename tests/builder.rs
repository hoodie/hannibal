#[test]
#[cfg(any(feature = "tokio", feature = "async-std"))]
fn invalid_builder_configurations() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/fails/builder_cant_recreate_streamhandler.rs");
}
