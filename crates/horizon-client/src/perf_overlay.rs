//! The performance overlay (Horizon's Overlay setting): frames per second,
//! frame, CPU and GPU times, memory, and a graph of recent frame times.
use crate::{
    graphics::Overlay,
    hud::{HudAtlas, HudBatch, rgba},
    text::Text,
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const HISTORY: usize = 160;
/// How often memory is read again.
const MEMORY_EVERY: Duration = Duration::from_secs(1);

/// Memory in MiB: the game's resident set, and the system's available and total memory (Linux
/// and Android, from `/proc`; none elsewhere).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Memory {
    pub process: Option<u64>,
    pub available: Option<u64>,
    pub total: Option<u64>,
}
impl Memory {
    /// From `/proc/self/status` and `/proc/meminfo`, whose sizes are in kB whatever the
    /// page size (Android's newer devices use 16 KiB pages).
    pub fn parse(status: &str, meminfo: &str) -> Self {
        let kib = |text: &str, key: &str| {
            text.lines().find_map(|line| {
                line.strip_prefix(key)?
                    .trim()
                    .strip_suffix("kB")?
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
        };
        let mib = |v: Option<u64>| v.map(|kib| kib / 1024);
        Memory {
            process: mib(kib(status, "VmRSS:")),
            available: mib(kib(meminfo, "MemAvailable:")),
            total: mib(kib(meminfo, "MemTotal:")),
        }
    }
    fn read() -> Self {
        let read = |path| std::fs::read_to_string(path).unwrap_or_default();
        Self::parse(&read("/proc/self/status"), &read("/proc/meminfo"))
    }
    /// The overlay's line, when anything was read.
    pub fn line(&self) -> Option<String> {
        let process = self.process?;
        Some(match (self.available, self.total) {
            (Some(available), Some(total)) => {
                format!("ram {process} MB  free {available}/{total} MB")
            }
            _ => format!("ram {process} MB"),
        })
    }
}

#[derive(Default)]
pub struct FrameGraph {
    /// Frame, CPU and GPU milliseconds of recent frames, oldest first.
    frames: VecDeque<(f32, f32, f32)>,
    memory: Memory,
    memory_read: Option<Instant>,
}
impl FrameGraph {
    pub fn record(&mut self, frame_ms: f32, cpu_ms: f32, gpu_ms: Option<f32>) {
        if self
            .memory_read
            .is_none_or(|at| at.elapsed() >= MEMORY_EVERY)
        {
            self.memory = Memory::read();
            self.memory_read = Some(Instant::now());
        }
        let gpu = gpu_ms.unwrap_or_else(|| self.frames.back().map_or(0.0, |f| f.2));
        if self.frames.len() == HISTORY {
            self.frames.pop_front();
        }
        self.frames.push_back((frame_ms, cpu_ms, gpu));
    }
    pub fn draw(&self, mode: Overlay, batch: &mut HudBatch, atlas: &HudAtlas) {
        if mode == Overlay::Off || self.frames.is_empty() {
            return;
        }
        let n = self.frames.len() as f32;
        let avg = |f: fn(&(f32, f32, f32)) -> f32| self.frames.iter().map(f).sum::<f32>() / n;
        let (frame, cpu, gpu) = (avg(|f| f.0), avg(|f| f.1), avg(|f| f.2));
        let worst = self.frames.iter().map(|f| f.0).fold(0.0f32, f32::max);
        let mut lines = vec![
            format!("{:.0} fps", 1000.0 / frame.max(0.001)),
            format!("frame {frame:.1} ms  max {worst:.1}"),
            format!("cpu {cpu:.1}  gpu {gpu:.1} ms"),
        ];
        lines.extend(self.memory.line());
        let width = 120.0f32.max(
            lines
                .iter()
                .map(|l| atlas.font.width(l))
                .fold(0.0, f32::max)
                + 6.0,
        );
        let x = batch.width - width - 2.0;
        // Little free memory on the system shows in amber, then red.
        let low = |available: u64, total: u64| available * 10 < total;
        let memory_color = match (self.memory.available, self.memory.total) {
            (Some(a), Some(t)) if a * 20 < t => rgba(240, 90, 80, 255),
            (Some(a), Some(t)) if low(a, t) => rgba(240, 200, 90, 255),
            _ => rgba(200, 200, 200, 255),
        };
        let graph = if mode == Overlay::Graph { 42.0 } else { 0.0 };
        batch.fill(
            atlas,
            x,
            2.0,
            width,
            4.0 + lines.len() as f32 * 10.0 + graph,
            rgba(0, 0, 0, 144),
        );
        for (i, line) in lines.iter().enumerate() {
            let color = if i == 0 {
                rgba(255, 255, 255, 255)
            } else if i == 3 {
                memory_color
            } else {
                rgba(200, 200, 200, 255)
            };
            batch.text_styled(
                atlas,
                &Text::literal(line.clone()),
                x + 3.0,
                4.0 + i as f32 * 10.0,
                color,
                true,
            );
        }
        if mode != Overlay::Graph {
            return;
        }
        // Bars of frame time on a 0..33 ms scale; lines at 60 and 30 fps.
        let (gy, gh) = (4.0 + lines.len() as f32 * 10.0 + 2.0, 36.0);
        let scale = gh / 33.3;
        let bar = (width - 6.0) / HISTORY as f32;
        for (i, &(ms, cpu, _)) in self.frames.iter().enumerate() {
            let height = (ms * scale).min(gh);
            let bx = x + 3.0 + i as f32 * bar;
            let color = if ms > 33.3 {
                rgba(240, 90, 80, 255)
            } else if ms > 16.7 {
                rgba(240, 200, 90, 255)
            } else {
                rgba(110, 210, 120, 255)
            };
            batch.fill(atlas, bx, gy + gh - height, bar.max(1.0), height, color);
            let cpu_height = (cpu * scale).min(height);
            batch.fill(
                atlas,
                bx,
                gy + gh - cpu_height,
                bar.max(1.0),
                cpu_height,
                rgba(80, 140, 230, 200),
            );
        }
        for target in [16.7, 33.3] {
            let ly = gy + gh - target * scale;
            batch.fill(
                atlas,
                x + 3.0,
                ly,
                width - 6.0,
                1.0,
                rgba(255, 255, 255, 90),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Memory;

    #[test]
    fn memory_is_read_in_kib_and_shown_in_mib() {
        let status = "Name:\thorizon\nVmPeak:\t 3000000 kB\nVmRSS:\t  845824 kB\nThreads:\t40\n";
        let meminfo = "MemTotal:        7864320 kB\nMemFree:          100000 kB\nMemAvailable:    3145728 kB\n";
        let m = Memory::parse(status, meminfo);
        assert_eq!(
            m,
            Memory {
                process: Some(826),
                available: Some(3072),
                total: Some(7680)
            }
        );
        assert_eq!(m.line().unwrap(), "ram 826 MB  free 3072/7680 MB");
        // Without /proc (Windows, macOS): no line.
        assert_eq!(Memory::parse("", "").line(), None);
        assert_eq!(Memory::parse(status, "").line().unwrap(), "ram 826 MB");
    }
}
