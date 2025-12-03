use uutests::new_ucmd;

#[test]
fn test_get_domainname_full() {
    let output = new_ucmd!().succeeds();
    assert!(!output.stdout_str().trim().is_empty());

    new_ucmd!()
        .succeeds()
        .stdout_contains(output.stdout_str().trim());
}

#[test]
fn test_invalid_arg() {
    new_ucmd!()
        .arg("--bad-param")
        .fails_with_code(1);
}