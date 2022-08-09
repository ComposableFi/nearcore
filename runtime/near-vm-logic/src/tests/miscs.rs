use crate::tests::fixtures::get_context;
use crate::tests::helpers::*;
use crate::tests::vm_logic_builder::VMLogicBuilder;
use crate::{map, ExtCosts};
use hex::FromHex;
use near_vm_errors::HostError;
use serde::{de::Error, Deserialize, Deserializer};
use serde_json::from_slice;
use std::{fmt::Display, fs};

#[test]
fn test_valid_utf8() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let string_bytes = "j ñ r'ø qò$`5 y'5 øò{%÷ `Võ%".as_bytes().to_vec();
    let len = string_bytes.len() as u64;
    logic.log_utf8(len, string_bytes.as_ptr() as _).expect("Valid utf-8 string_bytes");
    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs[0], String::from_utf8(string_bytes).unwrap());
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::log_base:  1,
        ExtCosts::log_byte: len,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf8_decoding_base: 1,
        ExtCosts::utf8_decoding_byte: len,
    });
}

#[test]
fn test_invalid_utf8() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let string_bytes = [128].to_vec();
    let len = string_bytes.len() as u64;
    assert_eq!(logic.log_utf8(len, string_bytes.as_ptr() as _), Err(HostError::BadUTF8.into()));
    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs.len(), 0);
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf8_decoding_base: 1,
        ExtCosts::utf8_decoding_byte: len,
    });
}

#[test]
fn test_valid_null_terminated_utf8() {
    let mut logic_builder = VMLogicBuilder::default();

    let mut string_bytes = "j ñ r'ø qò$`5 y'5 øò{%÷ `Võ%".as_bytes().to_vec();
    string_bytes.push(0u8);
    let bytes_len = string_bytes.len();
    logic_builder.config.limit_config.max_total_log_length = string_bytes.len() as u64;
    let mut logic = logic_builder.build(get_context(vec![], false));
    logic
        .log_utf8(u64::MAX, string_bytes.as_ptr() as _)
        .expect("Valid null-terminated utf-8 string_bytes");
    string_bytes.pop();
    let outcome = logic.compute_outcome_and_distribute_gas();
    let len = bytes_len as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::log_base: 1,
        ExtCosts::log_byte: len - 1,
        ExtCosts::read_memory_base: len,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf8_decoding_base: 1,
        ExtCosts::utf8_decoding_byte: len - 1,
    });
    assert_eq!(outcome.logs[0], String::from_utf8(string_bytes.clone()).unwrap());
}

#[test]
fn test_log_max_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let string_bytes = "j ñ r'ø qò$`5 y'5 øò{%÷ `Võ%".as_bytes().to_vec();
    let limit = (string_bytes.len() - 1) as u64;
    logic_builder.config.limit_config.max_total_log_length = limit;
    let mut logic = logic_builder.build(get_context(vec![], false));

    assert_eq!(
        logic.log_utf8(string_bytes.len() as _, string_bytes.as_ptr() as _),
        Err(HostError::TotalLogLengthExceeded { length: string_bytes.len() as _, limit }.into())
    );

    assert_costs(map! {
      ExtCosts::base: 1,
      ExtCosts::utf8_decoding_base: 1,
    });

    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs.len(), 0);
}

#[test]
fn test_log_total_length_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let string_bytes = "j ñ r'ø qò$`5 y'5 øò{%÷ `Võ%".as_bytes().to_vec();
    let num_logs = 10;
    let limit = string_bytes.len() as u64 * num_logs - 1;
    logic_builder.config.limit_config.max_total_log_length = limit;
    logic_builder.config.limit_config.max_number_logs = num_logs;
    let mut logic = logic_builder.build(get_context(vec![], false));

    for _ in 0..num_logs - 1 {
        logic
            .log_utf8(string_bytes.len() as _, string_bytes.as_ptr() as _)
            .expect("total is still under the limit");
    }
    assert_eq!(
        logic.log_utf8(string_bytes.len() as _, string_bytes.as_ptr() as _),
        Err(HostError::TotalLogLengthExceeded { length: limit + 1, limit }.into())
    );

    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs.len() as u64, num_logs - 1);
}

#[test]
fn test_log_number_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let string_bytes = "blabla".as_bytes().to_vec();
    let max_number_logs = 3;
    logic_builder.config.limit_config.max_total_log_length =
        (string_bytes.len() + 1) as u64 * (max_number_logs + 1);
    logic_builder.config.limit_config.max_number_logs = max_number_logs;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let len = string_bytes.len() as u64;
    for _ in 0..max_number_logs {
        logic
            .log_utf8(len, string_bytes.as_ptr() as _)
            .expect("Valid utf-8 string_bytes under the log number limit");
    }
    assert_eq!(
        logic.log_utf8(len, string_bytes.as_ptr() as _),
        Err(HostError::NumberOfLogsExceeded { limit: max_number_logs }.into())
    );

    assert_costs(map! {
        ExtCosts::base: max_number_logs + 1,
        ExtCosts::log_base: max_number_logs,
        ExtCosts::log_byte: len * max_number_logs,
        ExtCosts::read_memory_base: max_number_logs,
        ExtCosts::read_memory_byte: len * max_number_logs,
        ExtCosts::utf8_decoding_base: max_number_logs,
        ExtCosts::utf8_decoding_byte: len * max_number_logs,
    });

    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs.len() as u64, max_number_logs);
}

#[test]
fn test_log_utf16_number_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let string = "$ qò$`";
    let mut string_bytes: Vec<u8> = vec![0u8; 0];
    for u16_ in string.encode_utf16() {
        string_bytes.push(u16_ as u8);
        string_bytes.push((u16_ >> 8) as u8);
    }
    let max_number_logs = 3;
    logic_builder.config.limit_config.max_total_log_length =
        (string_bytes.len() + 1) as u64 * (max_number_logs + 1);
    logic_builder.config.limit_config.max_number_logs = max_number_logs;

    let mut logic = logic_builder.build(get_context(vec![], false));
    let len = string_bytes.len() as u64;
    for _ in 0..max_number_logs {
        logic
            .log_utf16(len, string_bytes.as_ptr() as _)
            .expect("Valid utf-16 string_bytes under the log number limit");
    }
    assert_eq!(
        logic.log_utf16(len, string_bytes.as_ptr() as _),
        Err(HostError::NumberOfLogsExceeded { limit: max_number_logs }.into())
    );

    assert_costs(map! {
        ExtCosts::base: max_number_logs + 1,
        ExtCosts::log_base: max_number_logs,
        ExtCosts::log_byte: string.len() as u64 * max_number_logs,
        ExtCosts::read_memory_base: max_number_logs,
        ExtCosts::read_memory_byte: len * max_number_logs,
        ExtCosts::utf16_decoding_base: max_number_logs,
        ExtCosts::utf16_decoding_byte: len * max_number_logs,
    });

    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs.len() as u64, max_number_logs);
}

#[test]
fn test_log_total_length_limit_mixed() {
    let mut logic_builder = VMLogicBuilder::default();
    let utf8_bytes = "abc".as_bytes().to_vec();

    let string = "abc";
    let mut utf16_bytes: Vec<u8> = vec![0u8; 0];
    for u16_ in string.encode_utf16() {
        utf16_bytes.push(u16_ as u8);
        utf16_bytes.push((u16_ >> 8) as u8);
    }

    let final_bytes = "abc".as_bytes().to_vec();

    let num_logs_each = 10;
    let limit = utf8_bytes.len() as u64 * num_logs_each
        + string.as_bytes().len() as u64 * num_logs_each
        + final_bytes.len() as u64
        - 1;
    logic_builder.config.limit_config.max_total_log_length = limit;
    logic_builder.config.limit_config.max_number_logs = num_logs_each * 2 + 1;
    let mut logic = logic_builder.build(get_context(vec![], false));

    for _ in 0..num_logs_each {
        logic
            .log_utf16(utf16_bytes.len() as _, utf16_bytes.as_ptr() as _)
            .expect("total is still under the limit");

        logic
            .log_utf8(utf8_bytes.len() as _, utf8_bytes.as_ptr() as _)
            .expect("total is still under the limit");
    }
    assert_eq!(
        logic.log_utf8(final_bytes.len() as _, final_bytes.as_ptr() as _),
        Err(HostError::TotalLogLengthExceeded { length: limit + 1, limit }.into())
    );

    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs.len() as u64, num_logs_each * 2);
}

#[test]
fn test_log_utf8_max_limit_null_terminated() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut string_bytes = "j ñ r'ø qò$`5 y'5 øò{%÷ `Võ%".as_bytes().to_vec();
    let limit = (string_bytes.len() - 1) as u64;
    logic_builder.config.limit_config.max_total_log_length = limit;
    let mut logic = logic_builder.build(get_context(vec![], false));

    string_bytes.push(0u8);
    assert_eq!(
        logic.log_utf8(u64::MAX, string_bytes.as_ptr() as _),
        Err(HostError::TotalLogLengthExceeded { length: limit + 1, limit }.into())
    );

    let len = string_bytes.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: len - 1 ,
        ExtCosts::read_memory_byte: len - 1,
        ExtCosts::utf8_decoding_base: 1,
    });

    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs.len(), 0);
}

#[test]
fn test_valid_log_utf16() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let string = "$ qò$`";
    let mut utf16_bytes: Vec<u8> = vec![0u8; 0];
    for u16_ in string.encode_utf16() {
        utf16_bytes.push(u16_ as u8);
        utf16_bytes.push((u16_ >> 8) as u8);
    }
    logic
        .log_utf16(utf16_bytes.len() as _, utf16_bytes.as_ptr() as _)
        .expect("Valid utf-16 string_bytes");

    let len = utf16_bytes.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf16_decoding_base: 1,
        ExtCosts::utf16_decoding_byte: len,
        ExtCosts::log_base: 1,
        ExtCosts::log_byte: string.len() as u64,
    });
    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs[0], string);
}

#[test]
fn test_valid_log_utf16_max_log_len_not_even() {
    let mut logic_builder = VMLogicBuilder::default();
    logic_builder.config.limit_config.max_total_log_length = 5;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let string = "ab";
    let mut utf16_bytes: Vec<u8> = Vec::new();
    for u16_ in string.encode_utf16() {
        utf16_bytes.push(u16_ as u8);
        utf16_bytes.push((u16_ >> 8) as u8);
    }
    utf16_bytes.extend_from_slice(&[0, 0]);
    logic.log_utf16(u64::MAX, utf16_bytes.as_ptr() as _).expect("Valid utf-16 string_bytes");

    let len = utf16_bytes.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: len / 2,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf16_decoding_base: 1,
        ExtCosts::utf16_decoding_byte: len - 2,
        ExtCosts::log_base: 1,
        ExtCosts::log_byte: string.len() as u64 ,
    });

    let string = "abc";
    let mut utf16_bytes: Vec<u8> = Vec::new();
    for u16_ in string.encode_utf16() {
        utf16_bytes.push(u16_ as u8);
        utf16_bytes.push((u16_ >> 8) as u8);
    }
    utf16_bytes.extend_from_slice(&[0, 0]);
    assert_eq!(
        logic.log_utf16(u64::MAX, utf16_bytes.as_ptr() as _),
        Err(HostError::TotalLogLengthExceeded {
            length: 6,
            limit: logic_builder.config.limit_config.max_total_log_length,
        }
        .into())
    );

    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 2,
        ExtCosts::read_memory_byte: 2 * 2,
        ExtCosts::utf16_decoding_base: 1,
    });
}

#[test]
fn test_log_utf8_max_limit_null_terminated_fail() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut string_bytes = "abcd".as_bytes().to_vec();
    string_bytes.push(0u8);
    logic_builder.config.limit_config.max_total_log_length = 3;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let res = logic.log_utf8(u64::MAX, string_bytes.as_ptr() as _);
    assert_eq!(res, Err(HostError::TotalLogLengthExceeded { length: 4, limit: 3 }.into()));
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: logic_builder.config.limit_config.max_total_log_length + 1,
        ExtCosts::read_memory_byte: logic_builder.config.limit_config.max_total_log_length + 1,
        ExtCosts::utf8_decoding_base: 1,
    });
}

#[test]
fn test_valid_log_utf16_null_terminated() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let string = "$ qò$`";
    let mut utf16_bytes: Vec<u8> = vec![0u8; 0];
    for u16_ in string.encode_utf16() {
        utf16_bytes.push(u16_ as u8);
        utf16_bytes.push((u16_ >> 8) as u8);
    }
    utf16_bytes.push(0);
    utf16_bytes.push(0);
    logic.log_utf16(u64::MAX, utf16_bytes.as_ptr() as _).expect("Valid utf-16 string_bytes");

    let len = utf16_bytes.len() as u64;
    let outcome = logic.compute_outcome_and_distribute_gas();
    assert_eq!(outcome.logs[0], string);
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: len / 2 ,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf16_decoding_base: 1,
        ExtCosts::utf16_decoding_byte: len - 2,
        ExtCosts::log_base: 1,
        ExtCosts::log_byte: string.len() as u64 ,
    });
}

#[test]
fn test_invalid_log_utf16() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let utf16: Vec<u16> = vec![0xD834, 0xDD1E, 0x006d, 0x0075, 0xD800, 0x0069, 0x0063];
    let mut utf16_bytes: Vec<u8> = vec![];
    for u16_ in utf16 {
        utf16_bytes.push(u16_ as u8);
        utf16_bytes.push((u16_ >> 8) as u8);
    }
    let res = logic.log_utf16(utf16_bytes.len() as _, utf16_bytes.as_ptr() as _);
    let len = utf16_bytes.len() as u64;
    assert_eq!(res, Err(HostError::BadUTF16.into()));
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf16_decoding_base: 1,
        ExtCosts::utf16_decoding_byte: len,
    });
}

#[test]
fn test_valid_log_utf16_null_terminated_fail() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let string = "$ qò$`";
    let mut utf16_bytes: Vec<u8> = vec![];
    for u16_ in string.encode_utf16() {
        utf16_bytes.push(u16_ as u8);
        utf16_bytes.push((u16_ >> 8) as u8);
    }
    utf16_bytes.push(0);
    utf16_bytes.push(0xD8u8); // Bad utf-16
    utf16_bytes.push(0);
    utf16_bytes.push(0);
    let res = logic.log_utf16(u64::MAX, utf16_bytes.as_ptr() as _);
    let len = utf16_bytes.len() as u64;
    assert_eq!(res, Err(HostError::BadUTF16.into()));
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: len / 2,
        ExtCosts::read_memory_byte: len,
        ExtCosts::utf16_decoding_base: 1,
        ExtCosts::utf16_decoding_byte: len - 2,
    });
}

#[test]
fn test_sha256() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";

    logic.sha256(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 32];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res,
        &[
            18, 176, 115, 156, 45, 100, 241, 132, 180, 134, 77, 42, 105, 111, 199, 127, 118, 112,
            92, 255, 88, 43, 83, 147, 122, 55, 26, 36, 42, 156, 160, 158,
        ]
    );
    let len = data.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::sha256_base: 1,
        ExtCosts::sha256_byte: len,
    });
}

#[test]
fn test_sha512() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";

    logic.sha512(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 64];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res,
        &[
            252, 57, 52, 83, 249, 244, 124, 10, 130, 53, 94, 254, 236, 56, 187, 138, 111, 90, 249,
            157, 88, 16, 31, 248, 200, 168, 184, 23, 173, 137, 10, 105, 115, 203, 114, 74, 204, 79,
            253, 230, 53, 231, 214, 42, 122, 223, 120, 116, 239, 238, 47, 12, 176, 219, 58, 230,
            55, 32, 221, 255, 214, 164, 183, 15
        ]
    );
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: data.len() as _,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::sha512_base: 1,
        ExtCosts::sha512_byte: data.len() as _,
    });
}

#[test]
fn test_sha512_truncated() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";

    logic.sha512_truncated(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 32];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res,
        &[
            252, 57, 52, 83, 249, 244, 124, 10, 130, 53, 94, 254, 236, 56, 187, 138, 111, 90, 249,
            157, 88, 16, 31, 248, 200, 168, 184, 23, 173, 137, 10, 105,
        ]
    );
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: data.len() as _,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::sha512_base: 1,
        ExtCosts::sha512_byte: data.len() as _,
    });
}

#[test]
fn test_sha3_512() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";

    logic.sha3_512(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 64];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res,
        &[
            133, 196, 48, 30, 203, 238, 194, 158, 186, 246, 118, 238, 42, 158, 212, 27, 178, 72,
            90, 229, 98, 108, 195, 221, 222, 161, 96, 219, 252, 99, 2, 48, 224, 15, 95, 220, 35,
            209, 27, 250, 43, 168, 250, 10, 21, 25, 97, 135, 235, 61, 5, 142, 182, 85, 36, 179, 23,
            126, 161, 14, 21, 118, 180, 231
        ]
    );
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: data.len() as _,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::sha3512_base: 1,
        ExtCosts::sha3512_byte: data.len() as _,
    });
}

#[test]
fn test_blake2_256() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";

    logic.blake2_256(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 32];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res,
        &[
            138, 169, 237, 5, 162, 52, 60, 246, 71, 130, 202, 107, 119, 4, 179, 36, 198, 44, 230,
            128, 158, 77, 83, 102, 154, 217, 73, 171, 215, 178, 27, 176
        ]
    );
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: data.len() as _,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::blake2_256_base: 1,
        ExtCosts::blake2_256_byte: data.len() as _,
    });
}

#[test]
fn test_keccak256() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";

    logic.keccak256(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 32];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res.as_slice(),
        &[
            104, 110, 58, 122, 230, 181, 215, 145, 231, 229, 49, 162, 123, 167, 177, 58, 26, 142,
            129, 173, 7, 37, 9, 26, 233, 115, 64, 102, 61, 85, 10, 159
        ]
    );
    let len = data.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::keccak256_base: 1,
        ExtCosts::keccak256_byte: len,
    });
}

#[test]
fn test_keccak512() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";

    logic.keccak512(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 64];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res,
        &[
            55, 134, 96, 137, 168, 122, 187, 95, 67, 76, 18, 122, 146, 11, 225, 106, 117, 194, 154,
            157, 48, 160, 90, 146, 104, 209, 118, 126, 222, 230, 200, 125, 48, 73, 197, 236, 123,
            173, 192, 197, 90, 153, 167, 121, 100, 88, 209, 240, 137, 86, 239, 41, 87, 128, 219,
            249, 136, 203, 220, 109, 46, 168, 234, 190
        ]
        .to_vec()
    );
    let len = data.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 64,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 64,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 64,
        ExtCosts::keccak512_base: 1,
        ExtCosts::keccak512_byte: len,
    });
}

#[test]
fn test_ripemd160() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));

    let data = b"tesdsst";
    logic.ripemd160(data.len() as _, data.as_ptr() as _, 0).unwrap();
    let res = &vec![0u8; 20];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(
        res,
        &[21, 102, 156, 115, 232, 3, 58, 215, 35, 84, 129, 30, 143, 86, 212, 104, 70, 97, 14, 225,]
    );
    let len = data.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: len,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 20,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 20,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 20,
        ExtCosts::ripemd160_base: 1,
        ExtCosts::ripemd160_block: 1,
    });
}

#[derive(Deserialize)]
struct EcrecoverTest {
    #[serde(with = "hex::serde")]
    m: [u8; 32],
    v: u8,
    #[serde(with = "hex::serde")]
    sig: [u8; 64],
    mc: bool,
    #[serde(deserialize_with = "deserialize_option_hex")]
    res: Option<[u8; 64]>,
}

fn deserialize_option_hex<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromHex,
    <T as FromHex>::Error: Display,
{
    Deserialize::deserialize(deserializer)
        .map(|v: Option<&str>| v.map(FromHex::from_hex).transpose().map_err(Error::custom))
        .and_then(|v| v)
}

#[test]
fn test_ecrecover() {
    for EcrecoverTest { m, v, sig, mc, res } in
        from_slice::<'_, Vec<_>>(fs::read("src/tests/ecrecover-tests.json").unwrap().as_slice())
            .unwrap()
    {
        let mut logic_builder = VMLogicBuilder::default();
        let mut logic = logic_builder.build(get_context(vec![], false));

        let b = logic
            .ecrecover(32, m.as_ptr() as _, 64, sig.as_ptr() as _, v as _, mc as _, 1)
            .unwrap();
        assert_eq!(b, res.is_some() as u64);

        if let Some(res) = res {
            assert_costs(map! {
                ExtCosts::read_memory_base: 2,
                ExtCosts::read_memory_byte: 96,
                ExtCosts::write_register_base: 1,
                ExtCosts::write_register_byte: 64,
                ExtCosts::ecrecover_base: 1,
            });
            let result = [0u8; 64];
            logic.read_register(1, result.as_ptr() as _).unwrap();
            assert_eq!(res, result);
        } else {
            assert_costs(map! {
                ExtCosts::read_memory_base: 2,
                ExtCosts::read_memory_byte: 96,
                ExtCosts::ecrecover_base: 1,
            });
        }

        reset_costs_counter();
    }
}

#[test]
fn test_hash256_register() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));
    let data = b"tesdsst";
    logic.wrapped_internal_write_register(1, data).unwrap();

    logic.sha256(u64::MAX, 1, 0).unwrap();
    let res = &vec![0u8; 32];
    logic.read_register(0, res.as_ptr() as _).unwrap();
    assert_eq!(
        res,
        &[
            18, 176, 115, 156, 45, 100, 241, 132, 180, 134, 77, 42, 105, 111, 199, 127, 118, 112,
            92, 255, 88, 43, 83, 147, 122, 55, 26, 36, 42, 156, 160, 158,
        ]
    );

    let len = data.len() as u64;
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 2,
        ExtCosts::read_register_byte: 32 + len,
        ExtCosts::write_register_base: 2,
        ExtCosts::write_register_byte: 32 + len,
        ExtCosts::sha256_base: 1,
        ExtCosts::sha256_byte: len,
    });
}

#[test]
fn test_key_length_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut key = "a".repeat(1024).as_bytes().to_vec();
    let val = b"hello";
    let limit = key.len() as u64;
    logic_builder.config.limit_config.max_length_storage_key = limit;
    let mut logic = logic_builder.build(get_context(vec![], false));
    // Under the limit. Valid calls.
    logic
        .storage_has_key(key.len() as _, key.as_ptr() as _)
        .expect("storage_has_key: key length is under the limit");
    logic
        .storage_write(key.len() as _, key.as_ptr() as _, val.len() as _, val.as_ptr() as _, 0)
        .expect("storage_read: key length is under the limit");
    logic
        .storage_read(key.len() as _, key.as_ptr() as _, 0)
        .expect("storage_read: key length is under the limit");
    logic
        .storage_remove(key.len() as _, key.as_ptr() as _, 0)
        .expect("storage_remove: key length is under the limit");
    // Over the limit. Invalid calls.
    key.push(b'a');
    assert_eq!(
        logic.storage_has_key(key.len() as _, key.as_ptr() as _),
        Err(HostError::KeyLengthExceeded { length: key.len() as _, limit }.into())
    );
    assert_eq!(
        logic.storage_write(
            key.len() as _,
            key.as_ptr() as _,
            val.len() as _,
            val.as_ptr() as _,
            0
        ),
        Err(HostError::KeyLengthExceeded { length: key.len() as _, limit }.into())
    );
    assert_eq!(
        logic.storage_read(key.len() as _, key.as_ptr() as _, 0),
        Err(HostError::KeyLengthExceeded { length: key.len() as _, limit }.into())
    );
    assert_eq!(
        logic.storage_remove(key.len() as _, key.as_ptr() as _, 0),
        Err(HostError::KeyLengthExceeded { length: key.len() as _, limit }.into())
    );
}

#[test]
fn test_value_length_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut val = "a".repeat(1024).as_bytes().to_vec();
    logic_builder.config.limit_config.max_length_storage_value = val.len() as u64;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let key = b"hello";
    logic
        .storage_write(key.len() as _, key.as_ptr() as _, val.len() as _, val.as_ptr() as _, 0)
        .expect("Value length is under the limit");
    val.push(b'a');
    assert_eq!(
        logic.storage_write(
            key.len() as _,
            key.as_ptr() as _,
            val.len() as _,
            val.as_ptr() as _,
            0
        ),
        Err(HostError::ValueLengthExceeded {
            length: val.len() as u64,
            limit: logic_builder.config.limit_config.max_length_storage_value
        }
        .into())
    );
}

#[test]
fn test_num_promises() {
    let mut logic_builder = VMLogicBuilder::default();
    let num_promises = 10;
    logic_builder.config.limit_config.max_promises_per_function_call_action = num_promises;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let account_id = b"alice";
    for _ in 0..num_promises {
        logic
            .promise_batch_create(account_id.len() as _, account_id.as_ptr() as _)
            .expect("Number of promises is under the limit");
    }
    assert_eq!(
        logic.promise_batch_create(account_id.len() as _, account_id.as_ptr() as _),
        Err(HostError::NumberPromisesExceeded {
            number_of_promises: num_promises + 1,
            limit: logic_builder.config.limit_config.max_promises_per_function_call_action
        }
        .into())
    );
}

#[test]
fn test_num_joined_promises() {
    let mut logic_builder = VMLogicBuilder::default();
    let num_deps = 10;
    logic_builder.config.limit_config.max_number_input_data_dependencies = num_deps;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let account_id = b"alice";
    let promise_id = logic
        .promise_batch_create(account_id.len() as _, account_id.as_ptr() as _)
        .expect("Number of promises is under the limit");
    for num in 0..num_deps {
        let promises = vec![promise_id; num as usize];
        logic
            .promise_and(promises.as_ptr() as _, promises.len() as _)
            .expect("Number of joined promises is under the limit");
    }
    let promises = vec![promise_id; (num_deps + 1) as usize];
    assert_eq!(
        logic.promise_and(promises.as_ptr() as _, promises.len() as _),
        Err(HostError::NumberInputDataDependenciesExceeded {
            number_of_input_data_dependencies: promises.len() as u64,
            limit: logic_builder.config.limit_config.max_number_input_data_dependencies,
        }
        .into())
    );
}

#[test]
fn test_num_input_dependencies_recursive_join() {
    let mut logic_builder = VMLogicBuilder::default();
    let num_steps = 10;
    logic_builder.config.limit_config.max_number_input_data_dependencies = 1 << num_steps;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let account_id = b"alice";
    let original_promise_id = logic
        .promise_batch_create(account_id.len() as _, account_id.as_ptr() as _)
        .expect("Number of promises is under the limit");
    let mut promise_id = original_promise_id;
    for _ in 1..num_steps {
        let promises = vec![promise_id, promise_id];
        promise_id = logic
            .promise_and(promises.as_ptr() as _, promises.len() as _)
            .expect("Number of joined promises is under the limit");
    }
    // The length of joined promises is exactly the limit (1024).
    let promises = vec![promise_id, promise_id];
    logic
        .promise_and(promises.as_ptr() as _, promises.len() as _)
        .expect("Number of joined promises is under the limit");

    // The length of joined promises exceeding the limit by 1 (total 1025).
    let promises = vec![promise_id, promise_id, original_promise_id];
    assert_eq!(
        logic.promise_and(promises.as_ptr() as _, promises.len() as _),
        Err(HostError::NumberInputDataDependenciesExceeded {
            number_of_input_data_dependencies: logic_builder
                .config
                .limit_config
                .max_number_input_data_dependencies
                + 1,
            limit: logic_builder.config.limit_config.max_number_input_data_dependencies,
        }
        .into())
    );
}

#[test]
fn test_return_value_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut val = "a".repeat(1024).as_bytes().to_vec();
    logic_builder.config.limit_config.max_length_returned_data = val.len() as u64;
    let mut logic = logic_builder.build(get_context(vec![], false));
    logic
        .value_return(val.len() as _, val.as_ptr() as _)
        .expect("Returned value length is under the limit");
    val.push(b'a');
    assert_eq!(
        logic.value_return(val.len() as _, val.as_ptr() as _),
        Err(HostError::ReturnedValueLengthExceeded {
            length: val.len() as u64,
            limit: logic_builder.config.limit_config.max_length_returned_data
        }
        .into())
    );
}

#[test]
fn test_contract_size_limit() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut code = "a".repeat(1024).as_bytes().to_vec();
    logic_builder.config.limit_config.max_contract_size = code.len() as u64;
    let mut logic = logic_builder.build(get_context(vec![], false));
    let account_id = b"alice";
    let promise_id = logic
        .promise_batch_create(account_id.len() as _, account_id.as_ptr() as _)
        .expect("Number of promises is under the limit");
    logic
        .promise_batch_action_deploy_contract(promise_id, code.len() as u64, code.as_ptr() as _)
        .expect("The length of the contract code is under the limit");
    code.push(b'a');
    assert_eq!(
        logic.promise_batch_action_deploy_contract(
            promise_id,
            code.len() as u64,
            code.as_ptr() as _
        ),
        Err(HostError::ContractSizeExceeded {
            size: code.len() as u64,
            limit: logic_builder.config.limit_config.max_contract_size
        }
        .into())
    );
}

#[test]
fn test_ed25519_verify() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));

    let signature: [u8; 64] = [
        145, 193, 203, 18, 114, 227, 14, 117, 33, 213, 121, 66, 130, 14, 25, 4, 36, 120, 46, 142,
        226, 215, 7, 66, 122, 112, 97, 30, 249, 135, 61, 165, 221, 249, 252, 23, 105, 40, 56, 70,
        31, 152, 236, 141, 154, 122, 207, 20, 75, 118, 79, 90, 168, 6, 221, 122, 213, 29, 126, 196,
        216, 104, 191, 6,
    ];

    let bad_signature: [u8; 64] = [1; 64];

    let public_key: [u8; 32] = [
        32, 122, 6, 120, 146, 130, 30, 37, 215, 112, 241, 251, 160, 196, 124, 17, 255, 75, 129, 62,
        84, 22, 46, 206, 158, 184, 57, 224, 118, 35, 26, 182,
    ];

    // 32 bytes message
    let message: [u8; 32] = [
        107, 97, 106, 100, 108, 102, 107, 106, 97, 108, 107, 102, 106, 97, 107, 108, 102, 106, 100,
        107, 108, 97, 100, 106, 102, 107, 108, 106, 97, 100, 115, 107,
    ];

    logic
        .ed25519_verify(
            signature.len() as _,
            signature.as_ptr() as _,
            message.len() as _,
            message.as_ptr() as _,
            public_key.len() as _,
            public_key.as_ptr() as _,
            0,
        )
        .unwrap();

    let res = &vec![0u8; 1];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(res.as_slice(), &[1]);

    logic
        .ed25519_verify(
            bad_signature.len() as _,
            bad_signature.as_ptr() as _,
            message.len() as _,
            message.as_ptr() as _,
            public_key.len() as _,
            public_key.as_ptr() as _,
            0,
        )
        .unwrap();
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(res.as_slice(), &[0]);
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: 32,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::ed25519_verify_base: 1,
        ExtCosts::ed25519_verify_byte: 32,
    });
}

#[test]
fn test_sr25519_verify() {
    let mut logic_builder = VMLogicBuilder::default();
    let mut logic = logic_builder.build(get_context(vec![], false));

    let message: [u8; 32] = [
        107, 97, 106, 100, 108, 102, 107, 106, 97, 108, 107, 102, 106, 97, 107, 108, 102, 106, 100,
        107, 108, 97, 100, 106, 102, 107, 108, 106, 97, 100, 115, 107,
    ];
    let signature: [u8; 64] = [
        106, 144, 17, 34, 142, 65, 191, 241, 233, 250, 132, 168, 204, 173, 122, 196, 118, 248, 159,
        159, 254, 37, 153, 84, 248, 104, 206, 217, 168, 65, 12, 74, 183, 134, 143, 30, 123, 61,
        112, 153, 244, 109, 199, 195, 164, 0, 7, 55, 26, 199, 164, 219, 147, 217, 157, 239, 198,
        108, 162, 246, 52, 49, 116, 132,
    ];

    let public_key: [u8; 32] = [
        190, 72, 112, 6, 182, 204, 56, 92, 5, 158, 148, 55, 136, 35, 90, 216, 30, 35, 86, 208, 210,
        66, 158, 72, 67, 25, 35, 217, 88, 145, 65, 113,
    ];

    logic
        .sr25519_verify(
            signature.len() as _,
            signature.as_ptr() as _,
            message.len() as _,
            message.as_ptr() as _,
            public_key.len() as _,
            public_key.as_ptr() as _,
            0,
        )
        .unwrap();

    let res = &vec![0u8; 1];
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(res.as_slice(), &[1]);

    logic
        .sr25519_verify(
            signature.len() as _,
            signature.as_ptr() as _,
            message.len() as _,
            [1; 32].as_ptr() as _,
            public_key.len() as _,
            public_key.as_ptr() as _,
            0,
        )
        .unwrap();
    logic.read_register(0, res.as_ptr() as _).expect("OK");
    assert_eq!(res.as_slice(), &[0]);
    assert_costs(map! {
        ExtCosts::base: 1,
        ExtCosts::read_memory_base: 1,
        ExtCosts::read_memory_byte: 32,
        ExtCosts::write_memory_base: 1,
        ExtCosts::write_memory_byte: 32,
        ExtCosts::read_register_base: 1,
        ExtCosts::read_register_byte: 32,
        ExtCosts::write_register_base: 1,
        ExtCosts::write_register_byte: 32,
        ExtCosts::sr25519_verify_base: 1,
        ExtCosts::sr25519_verify_byte: 32,
    });
}

fn populate_trie<'db, T>(
    db: &'db mut dyn trie_db::HashDB<T::Hash, trie_db::DBValue>,
    root: &'db mut trie_db::TrieHash<T>,
    v: &[(Vec<u8>, Vec<u8>)],
) -> trie_db::TrieDBMut<'db, T>
where
    T: trie_db::TrieConfiguration,
{
    use sp_trie::TrieMut;
    let mut t = trie_db::TrieDBMut::<T>::new(db, root);
    for i in 0..v.len() {
        let key: &[u8] = &v[i].0;
        let val: &[u8] = &v[i].1;
        t.insert(key, val).unwrap();
    }
    t
}

pub fn generate_trie_proof<'a, L, I, K, DB>(
    db: &DB,
    root: trie_db::TrieHash<L>,
    keys: I,
) -> Vec<Vec<u8>>
where
    L: trie_db::TrieConfiguration,
    I: IntoIterator<Item = &'a K>,
    K: 'a + AsRef<[u8]>,
    DB: hash_db::HashDBRef<L::Hash, trie_db::DBValue>,
{
    // Can use default layout (read only).
    let trie = trie_db::TrieDB::<L>::new(db, &root).unwrap();
    trie_db::proof::generate_proof(&trie, keys).unwrap()
}

#[test]
fn test_verify_membership_trie_proof() {
    let pairs = vec![
        (hex::encode("0102").into_bytes(), hex::encode("01").into_bytes()),
        (hex::encode("0203").into_bytes(), hex::encode("0405").into_bytes()),
    ];

    let mut memdb = memory_db::MemoryDB::<
        sp_runtime::traits::BlakeTwo256,
        memory_db::HashKey<_>,
        Vec<u8>,
    >::default();

    let mut root =
        trie_db::TrieHash::<sp_trie::LayoutV1<sp_runtime::traits::BlakeTwo256>>::default();
    populate_trie::<sp_trie::LayoutV1<sp_runtime::traits::BlakeTwo256>>(
        &mut memdb, &mut root, &pairs,
    );

    let included_key = hex::encode("0102").into_bytes();
    let proof =
        generate_trie_proof::<sp_trie::LayoutV1<_>, _, _, _>(&memdb, root, &[included_key.clone()]);

    // Verifying that the K was included into the trie should fail.
    assert!(sp_trie::verify_trie_proof::<
        sp_trie::LayoutV1<sp_runtime::traits::BlakeTwo256>,
        _,
        _,
        Vec<u8>,
    >(&root, &proof, &[(included_key.clone(), Some(hex::encode("01").into_bytes()))],)
    .is_ok());

    let number_of_proofs = proof.len();
    let proof_raw: Vec<u8> = proof
        .into_iter()
        .flat_map(|p| vec![(p.len() as u32).to_le_bytes().to_vec(), p].concat())
        .collect::<Vec<_>>();

    let mut logic_builder = VMLogicBuilder::free();
    let mut logic = logic_builder.build(get_context(vec![], false));

    logic
        .verify_membership_trie_proof(
            root.as_bytes().len() as _,
            root.as_bytes().as_ptr() as _,
            number_of_proofs as _,
            proof_raw.len() as _,
            proof_raw.as_ptr() as _,
            included_key.len() as _,
            included_key.as_ptr() as _,
            hex::encode("01").into_bytes().len() as _,
            hex::encode("01").into_bytes().as_ptr() as _,
        )
        .unwrap();
}

#[test]
fn test_verify_non_membership_trie_proof() {
    let pairs = vec![
        (hex::encode("0102").into_bytes(), hex::encode("01").into_bytes()),
        (hex::encode("0203").into_bytes(), hex::encode("0405").into_bytes()),
    ];

    let mut memdb = memory_db::MemoryDB::<
        sp_runtime::traits::BlakeTwo256,
        memory_db::HashKey<_>,
        Vec<u8>,
    >::default();

    let mut root =
        trie_db::TrieHash::<sp_trie::LayoutV1<sp_runtime::traits::BlakeTwo256>>::default();
    populate_trie::<sp_trie::LayoutV1<sp_runtime::traits::BlakeTwo256>>(
        &mut memdb, &mut root, &pairs,
    );

    let non_included_key = hex::encode("0909").into_bytes();
    let proof = generate_trie_proof::<sp_trie::LayoutV1<_>, _, _, _>(
        &memdb,
        root,
        &[non_included_key.clone()],
    );

    // Verifying that the K was not included into the trie should work.
    assert!(sp_trie::verify_trie_proof::<
        sp_trie::LayoutV1<sp_runtime::traits::BlakeTwo256>,
        _,
        _,
        Vec<u8>,
    >(&root, &proof, &[(non_included_key.clone(), None)],)
    .is_ok());

    // Verifying that the K was included into the trie should fail.
    assert!(sp_trie::verify_trie_proof::<
        sp_trie::LayoutV1<sp_runtime::traits::BlakeTwo256>,
        _,
        _,
        Vec<u8>,
    >(
        &root, &proof, &[(non_included_key.clone(), Some(hex::encode("1010").into_bytes()))],
    )
    .is_err());

    let number_of_proofs = proof.len();
    let proof_raw: Vec<u8> = proof
        .into_iter()
        .flat_map(|p| vec![(p.len() as u32).to_le_bytes().to_vec(), p].concat())
        .collect::<Vec<_>>();

    let mut logic_builder = VMLogicBuilder::free();
    let mut logic = logic_builder.build(get_context(vec![], false));

    logic
        .verify_non_membership_trie_proof(
            root.as_bytes().len() as _,
            root.as_bytes().as_ptr() as _,
            number_of_proofs as _,
            proof_raw.len() as _,
            proof_raw.as_ptr() as _,
            non_included_key.len() as _,
            non_included_key.as_ptr() as _,
        )
        .unwrap();

    logic
        .verify_non_membership_trie_proof(
            root.as_bytes().len() as _,
            root.as_bytes().as_ptr() as _,
            number_of_proofs as _,
            proof_raw.len() as _,
            proof_raw.as_ptr() as _,
            non_included_key.len() as _,
            non_included_key.as_ptr() as _,
        )
        .unwrap();
}
