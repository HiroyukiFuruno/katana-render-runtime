#[cfg(target_os = "macos")]
const BYTES_PER_RSS_UNIT: u64 = 1;
#[cfg(not(target_os = "macos"))]
const BYTES_PER_RSS_UNIT: u64 = 1024;

pub(crate) struct ProcessRss;

impl ProcessRss {
    pub(crate) fn peak_bytes() -> Result<u64, Box<dyn std::error::Error>> {
        /* WHY: 同じgetrusageのピークRSSを保ち、ABIとOSエラー処理は既存ライブラリへ委譲する。 */
        let usage = nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_SELF)?;
        let value = u64::try_from(usage.max_rss())?;
        Ok(value.saturating_mul(BYTES_PER_RSS_UNIT))
    }
}
