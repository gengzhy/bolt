//! 协议 V2 编解码（实施方案第四节）。
//!
//! 通用报文头：
//! `[4字节BE 包长度][1字节 指令码][8字节 会话/任务ID][载荷]`
//! 包长度 = 其后所有字节数（指令码+会话ID+载荷），上限 4GB（V1 实现保护上限 16MB）。
//! 所有多字节整数大端序；字符串为 2 字节长度前置的 UTF-8。

/// 控制面指令码（0x01~0x0F）。
pub mod op {
    pub const HELLO: u8 = 0x01;
    pub const PAIR_REQ: u8 = 0x02;
    pub const PAIR_RESP: u8 = 0x03;
    pub const TRANSFER_REQ: u8 = 0x04;
    pub const TRANSFER_RESP: u8 = 0x05;
    pub const FILE_META: u8 = 0x06;
    pub const FILE_META_ACK: u8 = 0x07;
    pub const CANCEL: u8 = 0x08;
    pub const ERROR: u8 = 0x09;
    pub const PING: u8 = 0x0A;
    pub const PONG: u8 = 0x0B;
    pub const BYE: u8 = 0x0C;
    /// 数据面指令码（0x20~0x2F）。
    pub const DATA: u8 = 0x20;
    pub const ACK: u8 = 0x21;
    pub const FILE_DONE: u8 = 0x22;
    pub const FILE_DONE_ACK: u8 = 0x23;
}

/// 哈希算法标识。
pub const HASH_BLAKE3: u8 = 0;
/// 设备类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum DeviceType {
    Windows = 1,
    Android = 2,
    Ios = 3,
    Linux = 4,
    Macos = 5,
    Unknown = 0,
}

impl DeviceType {
    pub fn from_u8(v: u8) -> DeviceType {
        match v {
            1 => DeviceType::Windows,
            2 => DeviceType::Android,
            3 => DeviceType::Ios,
            4 => DeviceType::Linux,
            5 => DeviceType::Macos,
            _ => DeviceType::Unknown,
        }
    }
}

/// 协议报文。
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Hello {
        proto_ver: u16,
        caps: u16,
        uuid: String,
        device_name: String,
        device_type: DeviceType,
        fingerprint: String,
    },
    PairReq {
        nonce: [u8; 16],
    },
    PairResp {
        accept: bool,
        nonce: [u8; 16],
    },
    /// 传输请求：携带全局唯一任务 ID、文件数、总大小、发送者名称。
    ///
    /// 【协议格式规范】：
    /// - task_uid (16B)：全局唯一的 128 位任务 ID（UUID），在重试、续传全生命周期保持恒定不变。
    /// - file_count (4B)：待传文件数。
    /// - total_size (8B)：整批文件总字节大小。
    /// - sender_name (2B 长度 + UTF-8 字符串)：发送端设备展示名称。
    TransferReq {
        task_uid: [u8; 16],
        file_count: u32,
        total_size: u64,
        sender_name: String,
    },
    /// 传输应答：对端用户接受或拒绝。
    ///
    /// 【协议格式规范】：
    /// - accept (1B)：是否接受传输。
    /// - task_uid (16B)：对应的全局任务 ID。
    TransferResp {
        accept: bool,
        task_uid: [u8; 16],
    },
    /// 单个文件的元数据（实施方案 7.2 逐文件握手）。
    ///
    /// 【协议格式规范】：
    /// - file_seq (4B)
    /// - size (8B)
    /// - mtime (8B)
    /// - rel_path (2B 长度 + UTF-8 字符串)
    /// - chunk_size (4B)
    /// - hash_algo (1B)
    /// 保持向前兼容的标准帧长，避免破坏 Android 与 Windows 端的二进制互通。
    FileMeta {
        file_seq: u32,
        size: u64,
        mtime: i64,
        rel_path: String,
        chunk_size: u32,
        hash_algo: u8,
    },
    FileMetaAck {
        file_seq: u32,
        accept: bool,
        ranges: Vec<(u64, u64)>,
    },
    Data {
        file_seq: u32,
        chunk_seq: u64,
        payload: bytes::Bytes,
    },
    Ack {
        file_seq: u32,
        acked_offset: u64,
    },
    FileDone {
        file_seq: u32,
        hash: [u8; 32],
    },
    FileDoneAck {
        file_seq: u32,
        ok: bool,
    },
    Cancel {
        reason: u32,
    },
    ErrorMsg {
        code: i32,
        detail: String,
    },
    Ping {
        seq: u64,
    },
    Pong {
        seq: u64,
    },
    Bye,
}

/// 编码失败/协议错误分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// 未知指令码（对端版本更新），应回 ERROR(-13) 并保持连接
    UnknownOpcode(u8),
    /// 载荷格式非法，应回 ERROR(-13) 并保持连接
    Malformed,
    /// 帧超长（保护上限），视为不可恢复
    TooLarge,
}

impl From<CodecError> for lt_utils::LtError {
    fn from(_: CodecError) -> Self {
        lt_utils::LtError::ProtocolIncompatible
    }
}

/// V1 实现保护上限：单帧 16MB（分片默认 1MB，协议上限 4GB 留待演进）。
pub const MAX_FRAME_LEN: u32 = 16 * 1024 * 1024;

/// 编码一条消息为完整帧（含 4 字节长度头）。
pub fn encode(msg: &Message, session: u64) -> Vec<u8> {
    let mut payload = Vec::new();
    let opcode = encode_payload(msg, &mut payload);
    let len = 1u32 + 8 + payload.len() as u32;
    let mut out = Vec::with_capacity(4 + len as usize);
    out.extend_from_slice(&len.to_be_bytes());
    out.push(opcode);
    out.extend_from_slice(&session.to_be_bytes());
    out.extend_from_slice(&payload);
    out
}

fn put_str(buf: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    let len = (b.len() as u16).to_be_bytes();
    buf.extend_from_slice(&len);
    buf.extend_from_slice(b);
}

fn encode_payload(msg: &Message, p: &mut Vec<u8>) -> u8 {
    match msg {
        Message::Hello {
            proto_ver,
            caps,
            uuid,
            device_name,
            device_type,
            fingerprint,
        } => {
            p.extend_from_slice(&proto_ver.to_be_bytes());
            p.extend_from_slice(&caps.to_be_bytes());
            put_str(p, uuid);
            put_str(p, device_name);
            p.push(*device_type as u8);
            put_str(p, fingerprint);
            op::HELLO
        }
        Message::PairReq { nonce } => {
            p.extend_from_slice(nonce);
            op::PAIR_REQ
        }
        Message::PairResp { accept, nonce } => {
            p.push(u8::from(*accept));
            p.extend_from_slice(nonce);
            op::PAIR_RESP
        }
        Message::TransferReq {
            task_uid,
            file_count,
            total_size,
            sender_name,
        } => {
            p.extend_from_slice(task_uid);
            p.extend_from_slice(&file_count.to_be_bytes());
            p.extend_from_slice(&total_size.to_be_bytes());
            put_str(p, sender_name);
            op::TRANSFER_REQ
        }
        Message::TransferResp { accept, task_uid } => {
            p.push(u8::from(*accept));
            p.extend_from_slice(task_uid);
            op::TRANSFER_RESP
        }
        Message::FileMeta {
            file_seq,
            size,
            mtime,
            rel_path,
            chunk_size,
            hash_algo,
        } => {
            p.extend_from_slice(&file_seq.to_be_bytes());
            p.extend_from_slice(&size.to_be_bytes());
            p.extend_from_slice(&mtime.to_be_bytes());
            put_str(p, rel_path);
            p.extend_from_slice(&chunk_size.to_be_bytes());
            p.push(*hash_algo);
            op::FILE_META
        }
        Message::FileMetaAck {
            file_seq,
            accept,
            ranges,
        } => {
            p.extend_from_slice(&file_seq.to_be_bytes());
            p.push(u8::from(*accept));
            p.extend_from_slice(&(ranges.len() as u32).to_be_bytes());
            for (s, e) in ranges {
                p.extend_from_slice(&s.to_be_bytes());
                p.extend_from_slice(&e.to_be_bytes());
            }
            op::FILE_META_ACK
        }
        Message::Data {
            file_seq,
            chunk_seq,
            payload,
        } => {
            p.extend_from_slice(&file_seq.to_be_bytes());
            p.extend_from_slice(&chunk_seq.to_be_bytes());
            p.extend_from_slice(payload);
            op::DATA
        }
        Message::Ack {
            file_seq,
            acked_offset,
        } => {
            p.extend_from_slice(&file_seq.to_be_bytes());
            p.extend_from_slice(&acked_offset.to_be_bytes());
            op::ACK
        }
        Message::FileDone { file_seq, hash } => {
            p.extend_from_slice(&file_seq.to_be_bytes());
            p.extend_from_slice(hash);
            op::FILE_DONE
        }
        Message::FileDoneAck { file_seq, ok } => {
            p.extend_from_slice(&file_seq.to_be_bytes());
            p.push(u8::from(*ok));
            op::FILE_DONE_ACK
        }
        Message::Cancel { reason } => {
            p.extend_from_slice(&reason.to_be_bytes());
            op::CANCEL
        }
        Message::ErrorMsg { code, detail } => {
            p.extend_from_slice(&code.to_be_bytes());
            put_str(p, detail);
            op::ERROR
        }
        Message::Ping { seq } => {
            p.extend_from_slice(&seq.to_be_bytes());
            op::PING
        }
        Message::Pong { seq } => {
            p.extend_from_slice(&seq.to_be_bytes());
            op::PONG
        }
        Message::Bye => op::BYE,
    }
}

// ---------------- 解码 ----------------

fn get_u16(d: &mut &[u8]) -> Result<u16, CodecError> {
    if d.len() < 2 {
        return Err(CodecError::Malformed);
    }
    let (h, rest) = d.split_at(2);
    *d = rest;
    Ok(u16::from_be_bytes([h[0], h[1]]))
}

fn get_u32(d: &mut &[u8]) -> Result<u32, CodecError> {
    if d.len() < 4 {
        return Err(CodecError::Malformed);
    }
    let (h, rest) = d.split_at(4);
    *d = rest;
    Ok(u32::from_be_bytes([h[0], h[1], h[2], h[3]]))
}

fn get_u64(d: &mut &[u8]) -> Result<u64, CodecError> {
    if d.len() < 8 {
        return Err(CodecError::Malformed);
    }
    let (h, rest) = d.split_at(8);
    *d = rest;
    Ok(u64::from_be_bytes([
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7],
    ]))
}

fn get_bytes<'a>(d: &mut &'a [u8], n: usize) -> Result<&'a [u8], CodecError> {
    if d.len() < n {
        return Err(CodecError::Malformed);
    }
    let (h, rest) = d.split_at(n);
    *d = rest;
    Ok(h)
}

fn get_str(d: &mut &[u8]) -> Result<String, CodecError> {
    let len = get_u16(d)? as usize;
    let b = get_bytes(d, len)?;
    String::from_utf8(b.to_vec()).map_err(|_| CodecError::Malformed)
}

fn get_bool(d: &mut &[u8]) -> Result<bool, CodecError> {
    Ok(get_bytes(d, 1)?[0] != 0)
}

fn get_i64(d: &mut &[u8]) -> Result<i64, CodecError> {
    Ok(get_u64(d)? as i64)
}

/// 解码载荷（指令码已取出）。未知指令码返回 `UnknownOpcode`。
pub fn decode(opcode: u8, payload: bytes::Bytes) -> Result<Message, CodecError> {
    let mut d = &payload[..];
    match opcode {
        op::HELLO => {
            let proto_ver = get_u16(&mut d)?;
            let caps = get_u16(&mut d)?;
            let uuid = get_str(&mut d)?;
            let device_name = get_str(&mut d)?;
            let dt = get_bytes(&mut d, 1)?[0];
            let fingerprint = get_str(&mut d)?;
            Ok(Message::Hello {
                proto_ver,
                caps,
                uuid,
                device_name,
                device_type: DeviceType::from_u8(dt),
                fingerprint,
            })
        }
        op::PAIR_REQ => {
            let n = get_bytes(&mut d, 16)?;
            Ok(Message::PairReq {
                nonce: n.try_into().unwrap(),
            })
        }
        op::PAIR_RESP => {
            let accept = get_bool(&mut d)?;
            let n = get_bytes(&mut d, 16)?;
            Ok(Message::PairResp {
                accept,
                nonce: n.try_into().unwrap(),
            })
        }
        op::TRANSFER_REQ => {
            // 【自适应双模解码】：严格保证向前与向后兼容，杜绝两端因报文长度差异断开
            // 新格式：16B task_uid + 4B file_count + 8B total_size + 2B str_len + str (payload 总长 == 30 + str_len)
            // 旧格式：4B file_count + 8B total_size + 2B str_len + str (payload 总长 == 14 + str_len)
            if d.len() >= 30 {
                let str_len = u16::from_be_bytes([d[28], d[29]]) as usize;
                if d.len() == 30 + str_len {
                    let mut task_uid = [0u8; 16];
                    task_uid.copy_from_slice(get_bytes(&mut d, 16)?);
                    let file_count = get_u32(&mut d)?;
                    let total_size = get_u64(&mut d)?;
                    let sender_name = get_str(&mut d)?;
                    return Ok(Message::TransferReq {
                        task_uid,
                        file_count,
                        total_size,
                        sender_name,
                    });
                }
            }
            if d.len() >= 14 {
                let str_len = u16::from_be_bytes([d[12], d[13]]) as usize;
                if d.len() == 14 + str_len {
                    let file_count = get_u32(&mut d)?;
                    let total_size = get_u64(&mut d)?;
                    let sender_name = get_str(&mut d)?;
                    // 旧版本协议兼容：由发送者、数量和大小派生确定性的回退 task_uid
                    let fallback_hash = blake3::hash(format!("{sender_name}|{file_count}|{total_size}").as_bytes());
                    let mut task_uid = [0u8; 16];
                    task_uid.copy_from_slice(&fallback_hash.as_bytes()[..16]);
                    return Ok(Message::TransferReq {
                        task_uid,
                        file_count,
                        total_size,
                        sender_name,
                    });
                }
            }
            // 兜底路径
            let mut task_uid = [0u8; 16];
            if d.len() >= 30 {
                task_uid.copy_from_slice(get_bytes(&mut d, 16)?);
            }
            let file_count = get_u32(&mut d)?;
            let total_size = get_u64(&mut d)?;
            let sender_name = get_str(&mut d)?;
            Ok(Message::TransferReq {
                task_uid,
                file_count,
                total_size,
                sender_name,
            })
        }
        op::TRANSFER_RESP => {
            let accept = get_bool(&mut d)?;
            let mut task_uid = [0u8; 16];
            if d.len() >= 16 {
                task_uid.copy_from_slice(get_bytes(&mut d, 16)?);
            }
            Ok(Message::TransferResp { accept, task_uid })
        }
        op::FILE_META => {
            let file_seq = get_u32(&mut d)?;
            let size = get_u64(&mut d)?;
            let mtime = get_i64(&mut d)?;
            let rel_path = get_str(&mut d)?;
            let chunk_size = get_u32(&mut d)?;
            let hash_algo = get_bytes(&mut d, 1)?[0];
            Ok(Message::FileMeta {
                file_seq,
                size,
                mtime,
                rel_path,
                chunk_size,
                hash_algo,
            })
        }
        op::FILE_META_ACK => {
            let file_seq = get_u32(&mut d)?;
            let accept = get_bool(&mut d)?;
            let count = get_u32(&mut d)? as usize;
            if count > d.len() / 16 {
                return Err(CodecError::Malformed);
            }
            let mut ranges = Vec::with_capacity(count);
            for _ in 0..count {
                let s = get_u64(&mut d)?;
                let e = get_u64(&mut d)?;
                ranges.push((s, e));
            }
            Ok(Message::FileMetaAck {
                file_seq,
                accept,
                ranges,
            })
        }
        op::DATA => {
            let file_seq = get_u32(&mut d)?;
            let chunk_seq = get_u64(&mut d)?;
            let chunk = payload.slice(12..);
            Ok(Message::Data {
                file_seq,
                chunk_seq,
                payload: chunk,
            })
        }
        op::ACK => {
            let file_seq = get_u32(&mut d)?;
            let acked_offset = get_u64(&mut d)?;
            Ok(Message::Ack {
                file_seq,
                acked_offset,
            })
        }
        op::FILE_DONE => {
            let file_seq = get_u32(&mut d)?;
            let h = get_bytes(&mut d, 32)?;
            Ok(Message::FileDone {
                file_seq,
                hash: h.try_into().unwrap(),
            })
        }
        op::FILE_DONE_ACK => {
            let file_seq = get_u32(&mut d)?;
            let ok = get_bytes(&mut d, 1)?[0] != 0;
            Ok(Message::FileDoneAck { file_seq, ok })
        }
        op::CANCEL => {
            let reason = get_u32(&mut d)?;
            Ok(Message::Cancel { reason })
        }
        op::ERROR => {
            let code = get_u32(&mut d)? as i32;
            let detail = get_str(&mut d)?;
            Ok(Message::ErrorMsg { code, detail })
        }
        op::PING => Ok(Message::Ping {
            seq: get_u64(&mut d)?,
        }),
        op::PONG => Ok(Message::Pong {
            seq: get_u64(&mut d)?,
        }),
        op::BYE => Ok(Message::Bye),
        unknown => Err(CodecError::UnknownOpcode(unknown)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(msg: Message) {
        let frame = encode(&msg, 0xDEAD_BEEF);
        let len = u32::from_be_bytes(frame[0..4].try_into().unwrap()) as usize;
        assert_eq!(frame.len(), 4 + len);
        assert_eq!(len, 1 + 8 + frame.len() - 4 - 9);
        let opcode = frame[4];
        let session = u64::from_be_bytes(frame[5..13].try_into().unwrap());
        assert_eq!(session, 0xDEAD_BEEF);
        let payload = bytes::Bytes::copy_from_slice(&frame[13..]);
        let decoded = decode(opcode, payload).expect("decode");
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_all_variants() {
        roundtrip(Message::Hello {
            proto_ver: 2,
            caps: 0,
            uuid: "u-1".into(),
            device_name: "我的电脑".into(),
            device_type: DeviceType::Windows,
            fingerprint: "aa:bb".into(),
        });
        roundtrip(Message::PairReq { nonce: [7; 16] });
        roundtrip(Message::PairResp {
            accept: true,
            nonce: [9; 16],
        });
        roundtrip(Message::TransferReq {
            task_uid: [1; 16],
            file_count: 3,
            total_size: 123456,
            sender_name: "手机".into(),
        });
        roundtrip(Message::TransferResp {
            accept: false,
            task_uid: [2; 16],
        });
        roundtrip(Message::FileMeta {
            file_seq: 2,
            size: 10_485_760,
            mtime: 1_700_000_000,
            rel_path: "dir/子目录/file.bin".into(),
            chunk_size: 1_048_576,
            hash_algo: HASH_BLAKE3,
        });
        roundtrip(Message::FileMetaAck {
            file_seq: 2,
            accept: true,
            ranges: vec![(0, 100), (200, 300)],
        });
        roundtrip(Message::Data {
            file_seq: 1,
            chunk_seq: 42,
            payload: bytes::Bytes::from_static(&[1, 2, 3]),
        });
        roundtrip(Message::Data {
            file_seq: 1,
            chunk_seq: 0,
            payload: bytes::Bytes::new(),
        });
        roundtrip(Message::Ack {
            file_seq: 5,
            acked_offset: 8 * 1024 * 1024,
        });
        roundtrip(Message::FileDone {
            file_seq: 9,
            hash: [0xAB; 32],
        });
        roundtrip(Message::FileDoneAck {
            file_seq: 9,
            ok: true,
        });
        roundtrip(Message::Cancel { reason: 9 });
        roundtrip(Message::ErrorMsg {
            code: -6,
            detail: "校验失败".into(),
        });
        roundtrip(Message::Ping { seq: 1 });
        roundtrip(Message::Pong { seq: 1 });
        roundtrip(Message::Bye);
    }

    #[test]
    fn unknown_opcode() {
        let err = decode(0x7F, bytes::Bytes::new()).unwrap_err();
        assert!(matches!(err, CodecError::UnknownOpcode(0x7F)));
    }

    #[test]
    fn malformed_payloads() {
        // HELLO 截断
        assert!(matches!(
            decode(op::HELLO, bytes::Bytes::from_static(&[0, 2])),
            Err(CodecError::Malformed)
        ));
        // FILE_DONE 哈希不足 32 字节
        assert!(matches!(
            decode(op::FILE_DONE, bytes::Bytes::from_static(&[0, 0, 0, 1, 1, 2, 3])),
            Err(CodecError::Malformed)
        ));
        // 非法 UTF-8 字符串
        let bad = [0u8, 2, 0xFF, 0xFE];
        assert!(matches!(
            decode(op::ERROR, bytes::Bytes::copy_from_slice(&bad[..])),
            Err(CodecError::Malformed)
        ));
        // FILE_META_ACK 区间数虚高
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.push(1);
        buf.extend_from_slice(&0xFF_FF_FF_FFu32.to_be_bytes());
        assert!(matches!(
            decode(op::FILE_META_ACK, bytes::Bytes::from(buf)),
            Err(CodecError::Malformed)
        ));
    }

    #[test]
    fn big_endian_wire() {
        let frame = encode(
            &Message::Ping {
                seq: 0x0102_0304_0506_0708,
            },
            0,
        );
        assert_eq!(frame[4], op::PING);
        assert_eq!(&frame[5..13], &[0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&frame[13..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_transfer_req_backwards_compatible() {
        // 模拟旧版本 14 字节 + UTF-8 字符串的载荷（无 16 字节 task_uid）
        let mut p = Vec::new();
        let file_count = 5u32;
        let total_size = 99999u64;
        let sender_name = "老版本Android客户端";
        p.extend_from_slice(&file_count.to_be_bytes());
        p.extend_from_slice(&total_size.to_be_bytes());
        put_str(&mut p, sender_name);

        let decoded = decode(op::TRANSFER_REQ, bytes::Bytes::from(p)).unwrap();
        match decoded {
            Message::TransferReq {
                task_uid,
                file_count: fc,
                total_size: ts,
                sender_name: sn,
            } => {
                assert_eq!(fc, 5);
                assert_eq!(ts, 99999);
                assert_eq!(sn, "老版本Android客户端");
                assert_ne!(task_uid, [0u8; 16]); // 派生出了有效的 fallback task_uid
            }
            _ => panic!("Expected Message::TransferReq"),
        }
    }
}
