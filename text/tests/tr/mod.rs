//
// Copyright (c) 2024 Jeff Garzik
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};

fn tr_test(args: &[&str], test_data: &str, expected_output: &str) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("tr"),
        args: str_args,
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
        expected_err: String::from(""),
        expected_exit_code: 0,
    });
}

#[test]
fn test_tr_1() {
    tr_test(&["abcd", "[]*]"], "abcd", "]]]]");
}

#[test]
fn tr_2() {
    tr_test(&["abc", "[%*]xyz"], "abc", "xyz");
}

#[test]
fn tr_3() {
    tr_test(&["abcd", "xy"], "abcde", "xyyye");
}

#[test]
fn tr_4() {
    tr_test(&["abcd", "x[y*]"], "abcde", "xyyye");
}

#[test]
fn tr_5() {
    tr_test(&["-s", "a-p", "%[.*]$"], "abcdefghijklmnop", "%.$");
}

#[test]
fn tr_6() {
    tr_test(&["-s", "a-p", "[.*]$"], "abcdefghijklmnop", ".$");
}

#[test]
fn tr_7() {
    tr_test(&["-s", "a-p", "%[.*]"], "abcdefghijklmnop", "%.");
}

#[test]
fn tr_a() {
    tr_test(&["-s", "[a-z]"], "aabbcc", "abc");
}

#[test]
fn tr_b() {
    tr_test(&["-s", "[a-c]"], "aabbcc", "abc");
}

#[test]
fn tr_c() {
    tr_test(&["-s", "[a-b]"], "aabbcc", "abcc");
}

#[test]
fn tr_d() {
    tr_test(&["-s", "[b-c]"], "aabbcc", "aabc");
}

#[test]
fn tr_f() {
    tr_test(&["-d", "[=[=]"], "[[[[[[[[]]]]]]]]", "]]]]]]]]");
}

#[test]
fn tr_g() {
    tr_test(&["-d", "[=]=]"], "[[[[[[[[]]]]]]]]", "[[[[[[[[");
}

#[test]
fn tr_h() {
    tr_test(&["-d", "[:xdigit:]"], "0123456789acbdefABCDEF", "");
}

#[test]
fn tr_i() {
    tr_test(
        &["-d", "[:xdigit:]"],
        "w0x1y2z3456789acbdefABCDEFz",
        "wxyzz",
    );
}

#[test]
fn tr_j() {
    tr_test(&["-d", "[:digit:]"], "0123456789", "");
}

#[test]
fn tr_k() {
    tr_test(&["-d", "[:digit:]"], "a0b1c2d3e4f5g6h7i8j9k", "abcdefghijk");
}

#[test]
fn tr_l() {
    tr_test(&["-d", "[:lower:]"], "abcdefghijklmnopqrstuvwxyz", "");
}

#[test]
fn tr_m() {
    tr_test(&["-d", "[:upper:]"], "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "");
}

#[test]
fn tr_n() {
    tr_test(
        &["-d", "[:lower:][:upper:]"],
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "",
    );
}

#[test]
fn tr_o() {
    tr_test(
        &["-d", "[:alpha:]"],
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "",
    );
}

#[test]
fn tr_p() {
    tr_test(
        &["-d", "[:alnum:]"],
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        "",
    );
}

#[test]
fn tr_q() {
    tr_test(
        &["-d", "[:alnum:]"],
        ".abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.",
        "..",
    );
}

#[test]
fn tr_r() {
    tr_test(
        &["-ds", "[:alnum:]", "."],
        ".abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.",
        ".",
    );
}

#[test]
fn tr_s() {
    tr_test(
        &["-c", "[:alnum:]", "\n"],
        "The big black fox jumped over the fence.",
        "The\nbig\nblack\nfox\njumped\nover\nthe\nfence\n",
    );
}

#[test]
fn tr_t() {
    tr_test(
        &["-c", "[:alnum:]", "[\n*]"],
        "The big black fox jumped over the fence.",
        "The\nbig\nblack\nfox\njumped\nover\nthe\nfence\n",
    );
}

#[test]
fn tr_u() {
    tr_test(&["-ds", "b", "a"], "aabbaa", "a");
}

#[test]
fn tr_v() {
    tr_test(
        &["-ds", "[:xdigit:]", "Z"],
        "ZZ0123456789acbdefABCDEFZZ",
        "Z",
    );
}

#[test]
fn tr_w() {
    tr_test(
        &["-ds", "\u{350}", "\u{345}"],
        "\u{300}\u{301}\u{377}\u{345}\u{345}\u{350}\u{345}",
        "\u{300}\u{301}\u{377}\u{345}",
    );
}

#[test]
fn tr_x() {
    tr_test(
        &["-s", "abcdefghijklmn", "[:*016]"],
        "abcdefghijklmnop",
        ":op",
    );
}

#[test]
fn tr_y() {
    tr_test(&["-d", "a-z"], "abc $code", " $");
}

#[test]
fn tr_z() {
    tr_test(&["-ds", "a-z", "$."], "a.b.c $$$$code\\", ". $\\");
}

#[test]
fn tr_range_a_a() {
    tr_test(&["a-a", "z"], "abc", "zbc");
}

#[test]
fn tr_upcase() {
    tr_test(&["[:lower:]", "[:upper:]"], "abcxyzABCXYZ", "ABCXYZABCXYZ");
}

#[test]
fn tr_dncase() {
    tr_test(&["[:upper:]", "[:lower:]"], "abcxyzABCXYZ", "abcxyzabcxyz");
}

#[test]
fn tr_rep_2() {
    tr_test(&["a[b*512]c", "1[x*]2"], "abc", "1x2");
}

#[test]
fn tr_rep_3() {
    tr_test(&["a[b*513]c", "1[x*]2"], "abc", "1x2");
}

#[test]
fn tr_o_rep_2() {
    tr_test(&["[b*010]cd", "[a*7]BC[x*]"], "bcd", "BCx");
}

#[test]
fn tr_ross_1a() {
    tr_test(&["-cs", "[:upper:]", "[X*]"], "AMZamz123.-+AMZ", "AMZXAMZ");
}

#[test]
fn tr_ross_1b() {
    tr_test(&["-cs", "[:upper:][:digit:]", "[Z*]"], "", "");
}

#[test]
fn tr_ross_2() {
    tr_test(&["-dcs", "[:lower:]", "n-rs-z"], "amzAMZ123.-+amz", "amzam");
}

#[test]
fn tr_ross_3() {
    tr_test(
        &["-ds", "[:xdigit:]", "[:alnum:]"],
        ".ZABCDEFzabcdefg.0123456788899.GG",
        ".Zzg..G",
    );
}

#[test]
fn tr_ross_4() {
    tr_test(&["-dcs", "[:alnum:]", "[:digit:]"], "", "");
}

#[test]
fn tr_ross_5() {
    tr_test(&["-dc", "[:lower:]"], "", "");
}

#[test]
fn tr_ross_6() {
    tr_test(&["-dc", "[:upper:]"], "", "");
}

#[test]
fn tr_repeat_0() {
    tr_test(&["abc", "[b*0]"], "abcd", "bbbd");
}

#[test]
fn tr_repeat_zeros() {
    tr_test(&["abc", "[b*00000000000000000000]"], "abcd", "bbbd");
}

#[test]
fn tr_repeat_compl() {
    tr_test(&["-c", "[a*65536]\n", "[b*]"], "abcd", "abbb");
}

#[test]
fn tr_repeat_xc() {
    tr_test(&["-C", "[a*65536]\n", "[b*]"], "abcd", "abbb");
}

#[test]
fn tr_no_abort_1() {
    tr_test(&["-c", "a", "[b*256]"], "abc", "abb");
}

#[test]
fn tr_d_space_n() {
    let input = "\
 54 68 69 73 20 69 73 20 61 20 73 75 69 74 65 20
 6f 66 20 52 75 73 74 2d 6e 61 74 69 76 65 20 63
 6f 72 65 20 63 6f 6d 6d 61 6e 64 20 6c 69 6e 65
 20 75 74 69 6c 74 69 65 73 20 28 63 70 2c 20 6d
 76 2c 0a 61 77 6b 2c 20 6d 61 6b 65 2c 20 76 69
 2c 20 2e 2e 2e 29 20 75 73 69 6e 67 20 50 4f 53
 49 58 2e 32 30 32 34 20 61 73 20 74 68 65 20 62
 61 73 65 6c 69 6e 65 20 73 70 65 63 69 66 69 63
 61 74 69 6f 6e 2e 0a
";

    let output = "\
546869732069732061207375697465206f6620527573742d6e617469766520636f726520636f6d6d616e64206c696e65207574696c74696573202863702c206d762c0a61776b2c206d616b652c2076692c202e2e2e29207573696e6720504f5349582e323032342061732074686520626173656c696e652073706563696669636174696f6e2e0a\
";

    tr_test(&["-d", r#" \n"#], input, output);
}

#[test]
fn tr_ignored_backslash() {
    tr_test(&["-d", r#"\z"#], "xyz", "xy");
}

#[test]
fn tr_escaped_backslash() {
    tr_test(&["-d", r#"\\"#], r#"a\b\c"#, "abc");
}
