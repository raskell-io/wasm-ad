//! Reference sound for WASM-AD (SPEC.md §14).
//!
//! [`frame`] is a pure function of memory, like the renderer: it reads the
//! sound registers, wave RAM, sample RAM and the host-owned sound state at
//! `SOUND_STATE`, writes one frame of audio, and writes back the state and the
//! registers the host updates. Hosts call it once per frame, after `update`.

use crate::addr::*;

/// Output sample rate in Hz.
pub const RATE: u32 = 48_000;
/// Stereo samples per frame (RATE / 60).
pub const SAMPLES: usize = 800;
/// The sound clock: 2^21 ticks per second (§14.2).
pub const CLOCK: u32 = 1 << 21;

// One output sample spans CLOCK / RATE = 16384 / 375 ticks: 43, plus one
// more whenever the remainder carries.
const TICKS: u32 = 16384 / 375;
const TICKS_REM: u32 = 16384 % 375;

fn r16(mem: &[u8], a: usize) -> u16 {
    u16::from_le_bytes([mem[a], mem[a + 1]])
}

fn w16(mem: &mut [u8], a: usize, v: u16) {
    mem[a..a + 2].copy_from_slice(&v.to_le_bytes());
}

/// A volume envelope (§14.3), shared by the square and noise channels.
#[derive(Default)]
struct Envelope {
    /// Bits 8–15 of the channel's envelope register, latched at start.
    params: u8,
    volume: u8,
    timer: u8,
}

impl Envelope {
    fn start(&mut self, reg: u16) {
        self.params = (reg >> 8) as u8;
        self.volume = self.params >> 4;
        self.timer = self.params & 7;
    }

    fn clock(&mut self) {
        let step = self.params & 7;
        if step == 0 {
            return;
        }
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = step;
            if self.params & 8 != 0 {
                self.volume = self.volume.saturating_add(1).min(15);
            } else {
                self.volume = self.volume.saturating_sub(1);
            }
        }
    }
}

#[derive(Default)]
struct Square {
    on: bool,
    step: u8,
    timer: u32,
    length: u16,
    env: Envelope,
    /// Channel 1's sweep: the swept rate and its countdown.
    sweep_rate: u16,
    sweep_timer: u8,
}

#[derive(Default)]
struct Wave {
    on: bool,
    pos: u8,
    timer: u32,
    length: u16,
}

#[derive(Default)]
struct Noise {
    on: bool,
    lfsr: u16,
    timer: u32,
    length: u16,
    env: Envelope,
}

#[derive(Default)]
struct Pcm {
    on: bool,
    pos: u16,
    acc: u16,
}

/// Everything the sound hardware remembers between frames, kept in memory at
/// `SOUND_STATE` (§14.8). Zeroed memory is the boot state. A cart may have
/// scribbled over it, so nothing here may panic on any bit pattern.
#[derive(Default)]
struct State {
    /// (next sample index × 16384) mod 375.
    rem: u16,
    /// The next tick's number, mod 8 × 4096.
    seq: u16,
    sq: [Square; 2],
    wave: Wave,
    noise: Noise,
    pcm: [Pcm; 2],
}

/// Little-endian (de)serialization of `State`, in field order: this order is
/// the layout SPEC.md §14.8 fixes, so don't reorder fields.
struct Cursor<'a> {
    bytes: &'a mut [u8],
    at: usize,
    write: bool,
}

impl Cursor<'_> {
    fn field<const N: usize>(&mut self, v: &mut [u8; N]) {
        let slot = &mut self.bytes[self.at..self.at + N];
        if self.write { slot.copy_from_slice(v) } else { v.copy_from_slice(slot) }
        self.at += N;
    }
    fn u8(&mut self, v: &mut u8) {
        let mut b = [*v];
        self.field(&mut b);
        *v = b[0];
    }
    fn bool(&mut self, v: &mut bool) {
        let mut b = *v as u8;
        self.u8(&mut b);
        *v = b != 0;
    }
    fn u16(&mut self, v: &mut u16) {
        let mut b = v.to_le_bytes();
        self.field(&mut b);
        *v = u16::from_le_bytes(b);
    }
    fn u32(&mut self, v: &mut u32) {
        let mut b = v.to_le_bytes();
        self.field(&mut b);
        *v = u32::from_le_bytes(b);
    }
    fn env(&mut self, e: &mut Envelope) {
        self.u8(&mut e.params);
        self.u8(&mut e.volume);
        self.u8(&mut e.timer);
    }
}

impl State {
    fn visit(&mut self, c: &mut Cursor) {
        c.u16(&mut self.rem);
        c.u16(&mut self.seq);
        for s in &mut self.sq {
            c.bool(&mut s.on);
            c.u8(&mut s.step);
            c.u32(&mut s.timer);
            c.u16(&mut s.length);
            c.env(&mut s.env);
            c.u16(&mut s.sweep_rate);
            c.u8(&mut s.sweep_timer);
        }
        let w = &mut self.wave;
        c.bool(&mut w.on);
        c.u8(&mut w.pos);
        c.u32(&mut w.timer);
        c.u16(&mut w.length);
        let n = &mut self.noise;
        c.bool(&mut n.on);
        c.u16(&mut n.lfsr);
        c.u32(&mut n.timer);
        c.u16(&mut n.length);
        c.env(&mut n.env);
        for p in &mut self.pcm {
            c.bool(&mut p.on);
            c.u16(&mut p.pos);
            c.u16(&mut p.acc);
        }
    }

    fn load(mem: &mut [u8]) -> Self {
        let mut s = Self::default();
        s.visit(&mut Cursor { bytes: &mut mem[SOUND_STATE..SOUND_STATE + SOUND_STATE_SIZE], at: 0, write: false });
        s
    }

    fn store(mut self, mem: &mut [u8]) {
        self.visit(&mut Cursor { bytes: &mut mem[SOUND_STATE..SOUND_STATE + SOUND_STATE_SIZE], at: 0, write: true });
    }
}

/// The sound registers as latched for this frame.
struct Regs {
    sweep: u16,
    duty: [u16; 2],
    ctl: [u16; 2],
    wave_sel: u16,
    wave_len_vol: u16,
    wave_ctl: u16,
    noise_env: u16,
    noise_ctl: u16,
    cnt_l: u16,
    cnt_h: u16,
}

impl Regs {
    fn read(mem: &[u8]) -> Self {
        Self {
            sweep: r16(mem, SOUND1CNT_L),
            duty: [r16(mem, SOUND1CNT_H), r16(mem, SOUND2CNT_L)],
            ctl: [r16(mem, SOUND1CNT_X), r16(mem, SOUND2CNT_H)],
            wave_sel: r16(mem, SOUND3CNT_L),
            wave_len_vol: r16(mem, SOUND3CNT_H),
            wave_ctl: r16(mem, SOUND3CNT_X),
            noise_env: r16(mem, SOUND4CNT_L),
            noise_ctl: r16(mem, SOUND4CNT_H),
            cnt_l: r16(mem, SOUNDCNT_L),
            cnt_h: r16(mem, SOUNDCNT_H),
        }
    }

    /// Sweep time and shift of channel 1; the sweep is active when both are nonzero.
    fn sweep(&self) -> (u8, u8) {
        (((self.sweep >> 4) & 7) as u8, (self.sweep & 7) as u8)
    }

    /// Ticks per duty step of square channel `i` (§14.3).
    fn square_period(&self, i: usize, st: &Square) -> u32 {
        let rate = match (i, self.sweep()) {
            (0, (t, n)) if t != 0 && n != 0 => st.sweep_rate & 0x7FF,
            _ => self.ctl[i] & 0x7FF,
        };
        2 * (2048 - rate as u32)
    }

    /// Ticks per shift of the noise register, or `None` when it does not shift (§14.5).
    fn noise_period(&self) -> Option<u32> {
        let (r, s) = ((self.noise_ctl & 7) as u32, (self.noise_ctl >> 4) & 15);
        (s < 14).then(|| if r == 0 { 4 } else { 8 * r } << s)
    }
}

const DUTY_HIGH: [u8; 4] = [1, 2, 4, 6];

/// Generates one frame of sound (§14): `mem` covers the hardware region
/// (at least `CART` bytes); `out` receives `SAMPLES` stereo pairs, left first.
pub fn frame(mem: &mut [u8], out: &mut [i16]) {
    assert!(mem.len() >= CART, "memory must cover the hardware region");
    assert!(out.len() >= SAMPLES * 2);
    let mut st = State::load(mem);
    let regs = Regs::read(mem);
    let enabled = r16(mem, SOUNDCNT_X) & 0x80 != 0;
    let pcm_cnt = [r16(mem, PCM_A + PCM_CNT), r16(mem, PCM_B + PCM_CNT)];

    // §14.2: starts and stops, then clear the bits.
    if enabled {
        for i in 0..2 {
            if regs.ctl[i] & 0x8000 != 0 {
                let s = &mut st.sq[i];
                s.on = true;
                s.step = 0;
                s.length = 64 - (regs.duty[i] & 63);
                s.env.start(regs.duty[i]);
                s.sweep_rate = regs.ctl[i] & 0x7FF;
                s.sweep_timer = regs.sweep().0;
                s.timer = regs.square_period(i, s);
            }
        }
        if regs.wave_ctl & 0x8000 != 0 && regs.wave_sel & 0x80 != 0 {
            let w = &mut st.wave;
            w.on = true;
            w.pos = 0;
            w.length = 256 - (regs.wave_len_vol & 255);
            w.timer = 2048 - (regs.wave_ctl & 0x7FF) as u32;
        }
        if regs.noise_ctl & 0x8000 != 0 {
            let n = &mut st.noise;
            n.on = true;
            n.lfsr = 0x7FFF;
            n.length = 64 - (regs.noise_env & 63);
            n.env.start(regs.noise_env);
            n.timer = regs.noise_period().unwrap_or(1);
        }
        for (p, cnt) in st.pcm.iter_mut().zip(pcm_cnt) {
            if cnt & 0x4000 != 0 {
                p.on = false;
            }
            if cnt & 0x8000 != 0 {
                *p = Pcm { on: true, pos: 0, acc: 0 };
            }
        }
    } else {
        st.sq.iter_mut().for_each(|s| s.on = false);
        st.wave.on = false;
        st.noise.on = false;
        st.pcm.iter_mut().for_each(|p| p.on = false);
    }
    if regs.wave_sel & 0x80 == 0 {
        st.wave.on = false;
    }
    for a in [SOUND1CNT_X, SOUND2CNT_H, SOUND3CNT_X, SOUND4CNT_H] {
        w16(mem, a, r16(mem, a) & 0x7FFF);
    }
    for base in [PCM_A, PCM_B] {
        w16(mem, base + PCM_CNT, r16(mem, base + PCM_CNT) & 0x3FFF);
    }

    let pcm_regs = [PCM_A, PCM_B].map(|b| PcmRegs::read(mem, b));
    let samples = &mem[SAMPLE_RAM..SAMPLE_RAM + SAMPLE_RAM_SIZE];
    let wave_ram: [u8; 32] = mem[WAVE_RAM..WAVE_RAM + 32].try_into().unwrap();

    for pair in out[..SAMPLES * 2].as_chunks_mut::<2>().0 {
        let ticks = TICKS + ((st.rem as u32 + TICKS_REM) >= 375) as u32;
        st.rem = ((st.rem as u32 + TICKS_REM) % 375) as u16;

        // §14.7: each PSG channel's level summed over the sample's ticks.
        let mut sums = [0i32; 4];
        for _ in 0..ticks {
            if st.seq.is_multiple_of(4096) {
                let step = st.seq / 4096;
                sequencer(&mut st, &regs, step);
            }
            st.seq = ((st.seq as u32 + 1) % (8 * 4096)) as u16;
            let levels = tick(&mut st, &regs, &wave_ram);
            for (s, l) in sums.iter_mut().zip(levels) {
                *s += l;
            }
        }

        let pcm: [i32; 2] = std::array::from_fn(|i| pcm_sample(&mut st.pcm[i], &pcm_regs[i], samples));
        if !enabled {
            pair.fill(0);
            continue;
        }
        for (side, v) in pair.iter_mut().enumerate() {
            // `right` is 0 for the left sample and 1 for the right one. In
            // SOUNDCNT_L the right side is bits 0–2 and 8–11, the left 4–6 and 12–15.
            let right = side as u16;
            let volume = ((regs.cnt_l >> ((1 - right) * 4)) & 7) as i32 + 1;
            let routed = (regs.cnt_l >> (8 + (1 - right) * 4)) & 15;
            let sum: i32 = (0..4).filter(|c| routed & (1 << c) != 0).map(|c| sums[c]).sum();
            let shift = 2 - (regs.cnt_h & 3).min(2) as i32;
            let mut mix = sum * volume * 32 / ((ticks as i32) << shift);
            for (i, p) in pcm.iter().enumerate() {
                // SOUNDCNT_H: A right bit 8, A left 9, B right 12, B left 13.
                if regs.cnt_h & (1 << (9 + i * 4 - right as usize)) != 0 {
                    mix += if regs.cnt_h & (4 << i) != 0 { *p } else { p >> 1 };
                }
            }
            *v = mix.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
    }

    // §14.2: the registers the host writes.
    let status = st.sq[0].on as u16
        | (st.sq[1].on as u16) << 1
        | (st.wave.on as u16) << 2
        | (st.noise.on as u16) << 3
        | (st.pcm[0].on as u16) << 4
        | (st.pcm[1].on as u16) << 5;
    w16(mem, SOUNDCNT_X, (r16(mem, SOUNDCNT_X) & !0x3F) | status);
    for (base, p) in [PCM_A, PCM_B].into_iter().zip(&st.pcm) {
        w16(mem, base + PCM_POS, p.pos);
    }
    st.store(mem);
}

/// The frame sequencer, at every tick that is a multiple of 4096 (§14.2).
fn sequencer(st: &mut State, regs: &Regs, step: u16) {
    if step.is_multiple_of(2) {
        let length = |on: &mut bool, length: &mut u16, ctl: u16| {
            if ctl & 0x4000 != 0 && *length > 0 {
                *length -= 1;
                if *length == 0 {
                    *on = false;
                }
            }
        };
        for (s, ctl) in st.sq.iter_mut().zip(regs.ctl) {
            length(&mut s.on, &mut s.length, ctl);
        }
        length(&mut st.wave.on, &mut st.wave.length, regs.wave_ctl);
        length(&mut st.noise.on, &mut st.noise.length, regs.noise_ctl);
    }
    if step == 2 || step == 6 {
        let (t, n) = regs.sweep();
        let s = &mut st.sq[0];
        if s.on && t != 0 && n != 0 {
            s.sweep_timer = s.sweep_timer.saturating_sub(1);
            if s.sweep_timer == 0 {
                s.sweep_timer = t;
                s.sweep_rate &= 0x7FF;
                let delta = s.sweep_rate >> n;
                let next = if regs.sweep & 8 != 0 { s.sweep_rate - delta } else { s.sweep_rate + delta };
                if next > 2047 {
                    s.on = false;
                } else {
                    s.sweep_rate = next;
                }
            }
        }
    }
    if step == 7 {
        st.sq.iter_mut().for_each(|s| s.env.clock());
        st.noise.env.clock();
    }
}

/// One tick: each PSG channel's level during it, then advance the timers.
fn tick(st: &mut State, regs: &Regs, wave_ram: &[u8; 32]) -> [i32; 4] {
    let mut levels = [0; 4];
    for i in 0..2 {
        let s = &mut st.sq[i];
        if !s.on {
            continue;
        }
        let v = s.env.volume as i32;
        let duty = ((regs.duty[i] >> 6) & 3) as usize;
        levels[i] = if s.step < DUTY_HIGH[duty] { v } else { -v };
        s.timer = s.timer.saturating_sub(1);
        if s.timer == 0 {
            s.step = (s.step + 1) % 8;
            s.timer = regs.square_period(i, s);
        }
    }

    let w = &mut st.wave;
    if w.on {
        let two_banks = regs.wave_sel & 0x20 != 0;
        let bank = ((regs.wave_sel >> 6) & 1) as usize;
        let pos = w.pos as usize % if two_banks { 64 } else { 32 };
        let byte = wave_ram[((bank + pos / 32) % 2) * 16 + (pos % 32) / 2];
        let s = if pos % 2 == 0 { byte >> 4 } else { byte & 15 } as i32;
        let m = if regs.wave_len_vol & 0x8000 != 0 { 3 } else { [0, 4, 2, 1][((regs.wave_len_vol >> 13) & 3) as usize] };
        levels[2] = ((2 * s - 15) * m) >> 2;
        w.timer = w.timer.saturating_sub(1);
        if w.timer == 0 {
            w.pos = ((pos + 1) % if two_banks { 64 } else { 32 }) as u8;
            w.timer = 2048 - (regs.wave_ctl & 0x7FF) as u32;
        }
    }

    let n = &mut st.noise;
    if n.on {
        let v = n.env.volume as i32;
        levels[3] = if n.lfsr & 1 == 0 { v } else { -v };
        if let Some(period) = regs.noise_period() {
            n.timer = n.timer.saturating_sub(1);
            if n.timer == 0 {
                let x = (n.lfsr ^ (n.lfsr >> 1)) & 1;
                n.lfsr = (n.lfsr >> 1) | (x << 14);
                if regs.noise_ctl & 8 != 0 {
                    n.lfsr = (n.lfsr & !(1 << 6)) | (x << 6);
                }
                n.timer = period;
            }
        }
    }
    levels
}

struct PcmRegs {
    start: usize,
    len: u16,
    loop_at: u16,
    rate: u32,
    volume: i32,
    looping: bool,
}

impl PcmRegs {
    fn read(mem: &[u8], base: usize) -> Self {
        let cnt = r16(mem, base + PCM_CNT);
        Self {
            start: r16(mem, base + PCM_SAD) as usize,
            len: r16(mem, base + PCM_LEN),
            loop_at: r16(mem, base + PCM_LOOP),
            rate: r16(mem, base + PCM_RATE) as u32,
            volume: (cnt & 0x7F).min(64) as i32,
            looping: cnt & 0x2000 != 0,
        }
    }
}

/// The sample channel's level for one output sample, then advance it (§14.6).
fn pcm_sample(p: &mut Pcm, regs: &PcmRegs, samples: &[u8]) -> i32 {
    // Past the end, loop or stop.
    let wrap = |p: &mut Pcm| {
        if p.pos >= regs.len {
            if regs.looping && regs.loop_at < regs.len {
                p.pos = regs.loop_at;
            } else {
                p.on = false;
            }
        }
    };
    if !p.on {
        return 0;
    }
    wrap(p);
    if !p.on {
        return 0;
    }
    let s = samples[(regs.start + p.pos as usize) % SAMPLE_RAM_SIZE] as i8 as i32;
    let mut acc = p.acc as u32 + regs.rate;
    while acc >= RATE {
        acc -= RATE;
        p.pos += 1;
        wrap(p);
        if !p.on {
            break;
        }
    }
    p.acc = acc as u16;
    s * regs.volume
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Hw(Vec<u8>);

    impl Hw {
        /// Sound on, every PSG channel on both sides at full volume, both
        /// sample channels on both sides at 100%.
        fn on() -> Self {
            let mut hw = Self(vec![0; CART]);
            hw.w(SOUNDCNT_X, 0x80).w(SOUNDCNT_L, 0xFF77).w(SOUNDCNT_H, 0x330E);
            hw
        }
        fn w(&mut self, a: usize, v: u16) -> &mut Self {
            w16(&mut self.0, a, v);
            self
        }
        fn r(&self, a: usize) -> u16 {
            r16(&self.0, a)
        }
        fn frame(&mut self) -> Vec<i16> {
            let mut out = vec![0; SAMPLES * 2];
            frame(&mut self.0, &mut out);
            out
        }
        fn frames(&mut self, n: usize) -> Vec<i16> {
            (0..n).flat_map(|_| self.frame()).collect()
        }
    }

    fn left(out: &[i16]) -> Vec<i16> {
        out.iter().step_by(2).copied().collect()
    }

    fn sign_changes(s: &[i16]) -> usize {
        s.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count()
    }

    #[test]
    fn silent_and_idle_while_disabled() {
        let mut hw = Hw::on();
        hw.w(SOUNDCNT_X, 0).w(SOUND2CNT_L, 0xF080).w(SOUND2CNT_H, 0x8000 | 1792);
        assert!(hw.frame().iter().all(|&s| s == 0));
        assert_eq!(hw.r(SOUNDCNT_X) & 0x3F, 0);
        assert_eq!(hw.r(SOUND2CNT_H), 1792, "the start bit is cleared");
    }

    #[test]
    fn square_wave_level_pitch_and_status() {
        let mut hw = Hw::on();
        // 50% duty, volume 15, rate 1792: 131072 / 256 = 512 Hz.
        hw.w(SOUND2CNT_L, 0xF080).w(SOUND2CNT_H, 0x8000 | 1792);
        let out = hw.frames(6);
        let l = left(&out);
        assert_eq!(out[0], out[1], "both sides");
        assert_eq!((*l.iter().max().unwrap(), *l.iter().min().unwrap()), (3840, -3840));
        // 512 Hz for 0.1 s: about 102 half periods.
        assert!((100..=104).contains(&sign_changes(&l)), "{}", sign_changes(&l));
        assert_eq!(hw.r(SOUNDCNT_X) & 0x3F, 0b10);

        // Routing: channel 2 off on the left.
        hw.w(SOUNDCNT_L, 0xDF77);
        let out = hw.frame();
        assert!(left(&out).iter().all(|&s| s == 0) && out[1..].iter().step_by(2).any(|&s| s != 0));
    }

    #[test]
    fn length_stops_a_channel() {
        let mut hw = Hw::on();
        // Length 64 − 62 = 2 clocks at 256 Hz: under one frame.
        hw.w(SOUND2CNT_L, 0xF080 | 62).w(SOUND2CNT_H, 0xC000 | 1792);
        let l = left(&hw.frame());
        assert_eq!(hw.r(SOUNDCNT_X) & 2, 0);
        assert!(l[..100].iter().any(|&s| s != 0) && l[500..].iter().all(|&s| s == 0));
        // Without the length enable it keeps going.
        hw.w(SOUND2CNT_H, 0x8000 | 1792);
        hw.frames(3);
        assert_eq!(hw.r(SOUNDCNT_X) & 2, 2);
    }

    #[test]
    fn envelope_fades_out() {
        let mut hw = Hw::on();
        // Volume 15, decreasing one step per 64 Hz clock: silent after 15/64 s.
        hw.w(SOUND2CNT_L, 0xF180).w(SOUND2CNT_H, 0x8000 | 1792);
        let peak = |out: Vec<i16>| out.iter().map(|s| s.abs()).max().unwrap();
        let (first, later) = (peak(hw.frame()), peak(hw.frames(6)));
        assert!(first > later && later > 0, "{first} {later}");
        hw.frames(10);
        assert_eq!(peak(hw.frame()), 0);
        assert_eq!(hw.r(SOUNDCNT_X) & 2, 2, "a silent channel is still on");
    }

    #[test]
    fn sweep_raises_pitch_until_it_overflows() {
        let mut hw = Hw::on();
        // Sweep time 1, increase, shift 1: rate × 1.5 per 128 Hz clock.
        hw.w(SOUND1CNT_L, 0x11).w(SOUND1CNT_H, 0xF080).w(SOUND1CNT_X, 0x8000 | 1024);
        let changes: Vec<usize> = (0..3).map(|_| sign_changes(&left(&hw.frame()))).collect();
        assert_eq!(hw.r(SOUNDCNT_X) & 1, 0, "1024 → 1536 → 2304 > 2047 stops it");
        assert!(changes[0] > 0 && changes[2] == 0, "{changes:?}");
    }

    #[test]
    fn wave_channel_plays_wave_ram() {
        let mut hw = Hw::on();
        // Bank 0: samples alternate 15 and 0.
        for i in 0..16 {
            hw.0[WAVE_RAM + i] = 0xF0;
        }
        hw.w(SOUND3CNT_L, 0x80).w(SOUND3CNT_H, 1 << 13).w(SOUND3CNT_X, 0x8000 | 1024);
        let l = left(&hw.frame());
        assert_eq!((*l.iter().max().unwrap(), *l.iter().min().unwrap()), (3840, -3840));
        // 25%: (2s − 15) / 4, rounded down.
        hw.w(SOUND3CNT_H, 3 << 13);
        let l = left(&hw.frame());
        assert_eq!((*l.iter().max().unwrap(), *l.iter().min().unwrap()), (3 * 256, -4 * 256));
        // Disabling the DAC stops it.
        hw.w(SOUND3CNT_L, 0);
        assert!(hw.frame().iter().all(|&s| s == 0));
        assert_eq!(hw.r(SOUNDCNT_X) & 4, 0);
    }

    #[test]
    fn noise_is_deterministic() {
        let run = || {
            let mut hw = Hw::on();
            hw.w(SOUND4CNT_L, 0xF000).w(SOUND4CNT_H, 0x8000 | 0x08 | 2 << 4 | 1);
            hw.frames(2)
        };
        let out = run();
        assert_eq!(out, run());
        assert!(sign_changes(&left(&out)) > 100);
    }

    #[test]
    fn sample_channels_play_loop_and_report_position() {
        let mut hw = Hw::on();
        hw.0[SAMPLE_RAM..SAMPLE_RAM + 4].copy_from_slice(&[100, 156, 50, 0]); // 100, −100, 50, 0
        hw.w(PCM_A + PCM_LEN, 4).w(PCM_A + PCM_RATE, 48000).w(PCM_A + PCM_CNT, 0x8000 | 64);
        let l = left(&hw.frame());
        assert_eq!(&l[..5], &[6400, -6400, 3200, 0, 0]);
        assert_eq!((hw.r(SOUNDCNT_X) & 0x10, hw.r(PCM_A + PCM_POS)), (0, 4));
        assert_eq!(hw.r(PCM_A + PCM_CNT), 64, "the start bit is cleared");

        // Half rate, looping from sample 2, 50% volume on channel B.
        hw.w(SOUNDCNT_H, 0x3306);
        hw.w(PCM_B + PCM_LEN, 4).w(PCM_B + PCM_LOOP, 2).w(PCM_B + PCM_RATE, 24000);
        hw.w(PCM_B + PCM_CNT, 0xA000 | 64);
        let l = left(&hw.frame());
        assert_eq!(&l[..12], &[3200, 3200, -3200, -3200, 1600, 1600, 0, 0, 1600, 1600, 0, 0]);
        assert_eq!(hw.r(SOUNDCNT_X) & 0x30, 0x20);

        hw.w(PCM_B + PCM_CNT, 0x4000 | 0x2000 | 64);
        hw.frame();
        assert_eq!(hw.r(SOUNDCNT_X) & 0x30, 0, "stopped");
    }

    #[test]
    fn state_lives_in_memory() {
        let mut hw = Hw::on();
        hw.w(SOUND1CNT_L, 0x1B).w(SOUND1CNT_H, 0xF240).w(SOUND1CNT_X, 0x8000 | 1500);
        hw.w(SOUND4CNT_L, 0xA300).w(SOUND4CNT_H, 0x8000 | 0x33);
        hw.frames(5);
        let mut copy = Hw(hw.0.clone());
        assert_eq!(hw.frames(20), copy.frames(20), "a memory snapshot resumes exactly");

        // 60 frames are exactly 2^21 ticks: the counters come back round.
        let mut hw = Hw::on();
        hw.frames(60);
        assert!(hw.0[SOUND_STATE..SOUND_STATE + 4].iter().all(|&b| b == 0));
    }

    #[test]
    fn state_layout_matches_the_spec() {
        let mut hw = Hw::on();
        hw.w(SOUND1CNT_H, 0xA380).w(SOUND1CNT_X, 0x8000 | 1000);
        hw.w(SOUND2CNT_L, 0x7180).w(SOUND2CNT_H, 0x8000 | 1000);
        hw.0[WAVE_RAM] = 0x12;
        hw.w(SOUND3CNT_L, 0x80).w(SOUND3CNT_H, 1 << 13).w(SOUND3CNT_X, 0x8000 | 1000);
        hw.w(SOUND4CNT_L, 0xC500).w(SOUND4CNT_H, 0x8000 | 0x21);
        hw.w(PCM_A + PCM_LEN, 4000).w(PCM_A + PCM_RATE, 12000).w(PCM_A + PCM_CNT, 0x8000 | 64);
        hw.w(PCM_B + PCM_LEN, 4000).w(PCM_B + PCM_RATE, 6000).w(PCM_B + PCM_CNT, 0x8000 | 64);
        hw.frame();
        let st = &hw.0[SOUND_STATE..];
        let u16_at = |o: usize| u16::from_le_bytes([st[o], st[o + 1]]);
        // §14.8: channel flags, latched envelope settings and sample positions.
        assert_eq!([st[0x04], st[0x12], st[0x20], st[0x28], st[0x34], st[0x39]], [1; 6]);
        assert_eq!((st[0x04 + 8], st[0x12 + 8], st[0x31]), (0xA3, 0x71, 0xC5));
        assert_eq!(u16_at(0x04 + 11), 1000, "channel 1's swept rate");
        assert_eq!((u16_at(0x35), u16_at(0x3A)), (200, 100), "800 samples at 12 and 6 kHz");
        assert_eq!((hw.r(PCM_A + PCM_POS), hw.r(PCM_B + PCM_POS)), (200, 100));
        assert!(u16_at(0x29) != 0x7FFF, "the noise register has shifted");
        assert!(st[0x3E..0x80].iter().all(|&b| b == 0), "reserved");
        // 800 samples cover ticks 0 to floor(800 × 16384 / 375) − 1 = 34951.
        assert_eq!((u16_at(0x00), u16_at(0x02)), ((800 * 16384 % 375) as u16, (34952 % 32768) as u16));
    }

    #[test]
    fn garbage_state_does_not_panic() {
        for fill in [0xFF, 0x80, 0x01] {
            let mut hw = Hw::on();
            hw.0[SOUND_REGS..SOUND_REGS_END].fill(fill);
            hw.0[SOUND_STATE..SOUND_STATE + SOUND_STATE_SIZE].fill(fill);
            hw.0[SAMPLE_RAM..].fill(fill);
            hw.frames(3);
        }
    }
}
