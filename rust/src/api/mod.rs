//! Dart/Rust FRB 边界层 / Dart–Rust FRB boundary layer.
//!
//! 本模块是 Flutter 侧 [`PlayerCommandPort`](../../lib/src/player/command_port.dart) 在 Rust 端的
//! 直接对应：[`player`] 提供生命周期与播放控制，[`types`] 定义跨语言 DTO，
//! [`simple`] 负责 FRB 初始化钩子。
//!
//! This module is the Rust counterpart of Dart's `PlayerCommandPort`:
//! [`player`] for lifecycle/controls, [`types`] for cross-language DTOs,
//! [`simple`] for the FRB init hook.

pub mod player;
pub mod simple;
pub mod types;
