//! 帧式读写管道：在任意 tokio AsyncRead/AsyncWrite 上收发协议帧。
//!
//! QUIC stream 与 TCP+TLS 流统一经此读写，上层无需感知传输差异。

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use lt_utils::{LtError, LtResult};

use crate::protocol::{self, CodecError, Message, MAX_FRAME_LEN};

/// 收到的一帧（解码成功或携带解码错误——未知指令需回 ERROR 且保持连接）。
pub enum Incoming {
    Msg { session: u64, msg: Message },
    Bad { session: u64, err: CodecError },
}

/// 读取一帧。EOF 返回 Ok(None)。
pub async fn read_frame<R: AsyncRead + Unpin + ?Sized>(r: &mut R) -> LtResult<Option<Incoming>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_be_bytes(len_buf);
    if !(9..=MAX_FRAME_LEN).contains(&len) {
        return Err(LtError::ProtocolIncompatible);
    }
    let mut body = bytes::BytesMut::with_capacity(len as usize);
    while body.len() < len as usize {
        if r.read_buf(&mut body).await.map_err(LtError::from)? == 0 {
            return Ok(None);
        }
    }
    let body = body.freeze();

    let opcode = body[0];
    let session = u64::from_be_bytes(body[1..9].try_into().unwrap());
    let payload = body.slice(9..);
    match protocol::decode(opcode, payload) {
        Ok(msg) => Ok(Some(Incoming::Msg { session, msg })),
        Err(err) => Ok(Some(Incoming::Bad { session, err })),
    }
}

/// 写入一帧（含长度头）。
pub async fn write_frame<W: AsyncWrite + Unpin + ?Sized>(
    w: &mut W,
    session: u64,
    msg: &Message,
) -> LtResult<()> {
    match msg {
        Message::Data {
            file_seq,
            chunk_seq,
            payload,
        } => {
            // DATA 帧两段向量化写入：头部（25字节）与 payload 分离写入，
            // 杜绝 1MB payload 在 protocol::encode 中重复拷贝和新分配
            let payload_len = payload.len();
            let total_len = 1 + 8 + 4 + 8 + payload_len as u32;
            let mut header = [0u8; 4 + 1 + 8 + 4 + 8];
            header[0..4].copy_from_slice(&total_len.to_be_bytes());
            header[4] = protocol::op::DATA;
            header[5..13].copy_from_slice(&session.to_be_bytes());
            header[13..17].copy_from_slice(&file_seq.to_be_bytes());
            header[17..25].copy_from_slice(&chunk_seq.to_be_bytes());

            w.write_all(&header).await.map_err(LtError::from)?;
            w.write_all(payload).await.map_err(LtError::from)?;
            Ok(())
        }
        _ => {
            let frame = protocol::encode(msg, session);
            w.write_all(&frame).await.map_err(LtError::from)?;
            Ok(())
        }
    }
}

/// 半双工拆分管道（读半部/写半部类型擦除）。
pub struct Pipe {
    pub reader: Box<dyn AsyncRead + Unpin + Send>,
    pub writer: Box<dyn AsyncWrite + Unpin + Send>,
}
