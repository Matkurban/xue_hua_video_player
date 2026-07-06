#[cfg(target_os = "android")]
use anyhow::{anyhow, Result};

#[cfg(target_os = "android")]
use crate::asset_resolver::AssetByteSource;

/// Opens a Flutter asset via `AssetManager.openFd` and returns a byte source.
#[cfg(target_os = "android")]
pub fn open_asset_fd(asset_key: &str) -> Result<AssetByteSource> {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    use jni::{jni_sig, jni_str};
    use jni::objects::{JLongArray, JObject, JValue};

    let (fd, start, length) = crate::platform_view_android::with_jni_env(|env| {
        let class = env.find_class(jni_str!(
            "com/flutter_rust_bridge/xue_hua_video_player/FlutterAssetHelper"
        ))?;
        let jkey = env
            .new_string(asset_key)
            .map_err(|e| anyhow!("new_string: {e}"))?;
        let args = [JValue::Object(&JObject::from(jkey))];
        let result = env.call_static_method(
            class,
            jni_str!("openAssetFd"),
            jni_sig!("(Ljava/lang/String;)[J"),
            &args,
        )?;
        let arr_obj = result.l().map_err(|e| anyhow!("result: {e}"))?;
        // SAFETY: Java returns `long[]`.
        let long_arr = unsafe { JLongArray::from_raw(env, arr_obj.as_raw() as jni::sys::jarray) };
        let len = long_arr
            .len(env)
            .map_err(|e| anyhow!("array len: {e}"))?;
        if len < 3 {
            return Err(anyhow!("openAssetFd returned short array"));
        }
        let mut buf = [0i64; 3];
        long_arr
            .get_region(env, 0, &mut buf)
            .map_err(|e| anyhow!("get_region: {e}"))?;
        let fd = buf[0] as i32;
        if fd < 0 {
            return Err(anyhow!("asset fd unavailable for {asset_key}"));
        }
        Ok((fd, buf[1] as u64, buf[2] as u64))
    })?;

    // SAFETY: fd detached from AssetFileDescriptor in Java helper.
    let file = unsafe { File::from_raw_fd(fd) };
    Ok(AssetByteSource::AndroidFd {
        file,
        start,
        length,
        position: 0,
    })
}
