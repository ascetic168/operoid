//! 工廠轉換的**公平調度閘門**（企業版多工核心；P4 工廠多工）。
//!
//! 使用者規格：工廠文件轉換做成多工，全域同時執行的檔案工作 ≤ `max`
//! （`factory_concurrency`，預設 4，可設定）；並在名額競爭時引入公平調度——
//! **飽和時每位遠端使用者至多 1 線程**。
//!
//! 落地語意（授予時裁定，不中斷已在執行者）：
//!
//! 1. **全域上限**：同時持有的名額 < `max`（每次呼叫取當下設定——熱生效）。
//! 2. **飢餓優先**：有名額釋出時，**尚未持有任何名額**的使用者（FIFO）先取得；
//!    使用者數 ≥ max 的飽和態即收斂為「每位使用者 1 線程」。
//! 3. **無競爭不浪費**：所有等待者都已有名額在跑（無人挨餓）時，才輪到
//!    「擴展」——單一使用者可展開到 max（個人版單機滿速即此形）。
//!
//! 等待者以 oneshot 授予；請求端取消（連線斷開、task abort）時其等待者由
//! `tx.is_closed()` 清掃、已授予名額隨 `Permit` drop 釋出——不外洩、不卡死。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// 一個等待名額的排隊項。
struct Waiter {
    user: String,
    tx: tokio::sync::oneshot::Sender<()>,
}

#[derive(Default)]
struct SchedState {
    /// 全域上限（每次 acquire 以當下設定覆寫——熱生效，最新值為準）。
    max: usize,
    active: usize,
    per_user: HashMap<String, usize>,
    queue: VecDeque<Waiter>,
}

/// 可複製的調度閘門（內部狀態共用）。oserver 持一個全域實例；
/// 個人版不經此（`run_core` 直接本地 buffered 並發）。
#[derive(Clone, Default)]
pub struct FactoryGate {
    inner: Arc<Mutex<SchedState>>,
}

impl FactoryGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// 執行一個檔案工作：先依公平規則取得名額，再跑 `job`；離開（含 panic／
    /// task abort）時隨 `Permit` drop 自動釋出名額並喚醒下一位。
    pub async fn run<T>(&self, user: &str, max: usize, job: impl std::future::Future<Output = T>) -> T {
        let permit = self.acquire(user, max).await;
        let out = job.await;
        drop(permit);
        out
    }

    async fn acquire(&self, user: &str, max: usize) -> Permit {
        loop {
            let rx = {
                let mut st = Self::lock(&self.inner);
                st.max = max.max(1);
                let (tx, rx) = tokio::sync::oneshot::channel();
                st.queue.push_back(Waiter { user: user.to_string(), tx });
                Self::dispatch_locked(&mut st);
                rx
            };
            match rx.await {
                Ok(()) => return Permit { gate: self.clone(), user: user.to_string() },
                // 授予端不會 drop sender（同一鎖內成對）；保險起見重新排隊。
                Err(_) => continue,
            }
        }
    }

    /// 授予掃描：先清掉已取消的等待者；有名額時先找「零持有」使用者（FIFO，
    /// 飢餓優先），全都已持有才允許隊首擴展（無競爭 → 單人展開到 max）。
    fn dispatch_locked(st: &mut SchedState) {
        st.queue.retain(|w| !w.tx.is_closed());
        while st.active < st.max {
            let idx = st
                .queue
                .iter()
                .position(|w| st.per_user.get(&w.user).copied().unwrap_or(0) == 0)
                .or(if st.queue.is_empty() { None } else { Some(0) });
            let Some(idx) = idx else { break };
            let w = st.queue.remove(idx).expect("idx 來自當下佇列");
            st.active += 1;
            *st.per_user.entry(w.user).or_default() += 1;
            let _ = w.tx.send(());
        }
    }

    fn lock(inner: &Mutex<SchedState>) -> std::sync::MutexGuard<'_, SchedState> {
        // 調度器不得因中毒鎖拖垮服務：poison 時取回內部狀態續用。
        inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// 已取得的名額。drop 時釋出（active／per_user 遞減）並重新授予掃描。
pub struct Permit {
    gate: FactoryGate,
    user: String,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut st = FactoryGate::lock(&self.gate.inner);
        st.active = st.active.saturating_sub(1);
        if let Some(c) = st.per_user.get_mut(&self.user) {
            *c = c.saturating_sub(1);
            if *c == 0 {
                st.per_user.remove(&self.user);
            }
        }
        FactoryGate::dispatch_locked(&mut st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{Context, Poll};

    type AcqFut<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Permit> + 'a>>;

    /// 非阻塞探測（noop waker 直接 poll）：true＝仍未授予（排隊中）。
    fn probe_pending(f: &mut AcqFut<'_>) -> bool {
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());
        matches!(f.as_mut().poll(&mut cx), Poll::Pending)
    }

    /// 授予並**持有**名額（Permit 不 drop——drop 即釋出）。
    fn grant(mut f: AcqFut<'_>, what: &str) -> Permit {
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(p) => p,
            Poll::Pending => panic!("{what} 應已授予"),
        }
    }

    /// 內部計數快照（測試觀測用）。
    fn snapshot(gate: &FactoryGate) -> (usize, Vec<(String, usize)>) {
        let st = FactoryGate::lock(&gate.inner);
        let mut pu: Vec<_> = st.per_user.iter().map(|(k, c)| (k.clone(), *c)).collect();
        pu.sort();
        (st.active, pu)
    }

    /// 單一使用者無競爭 → 展開到 max（產能不閒置）；超出的排隊，釋出即補位。
    #[test]
    fn single_user_fills_max() {
        let gate = FactoryGate::new();
        let mut held: Vec<Permit> = Vec::new();
        for _ in 0..4 {
            let f: AcqFut<'_> = Box::pin(gate.acquire("solo", 4));
            held.push(grant(f, "前 4 個"));
        }
        assert_eq!(snapshot(&gate).0, 4, "單人展開到 max");
        let mut q5: AcqFut<'_> = Box::pin(gate.acquire("solo", 4));
        assert!(probe_pending(&mut q5), "第 5 個應排隊");
        drop(held.pop());
        let p5 = grant(q5, "釋出後第 5 個補位");
        drop(p5);
    }

    /// 全域上限絕對不破：多使用者混流，僅 max 個同時授予；其餘排隊。
    #[test]
    fn global_max_respected() {
        let gate = FactoryGate::new();
        let users: Vec<String> = (0..5).map(|k| format!("u{k}")).collect();
        let mut live: Vec<Permit> = Vec::new();
        let mut pending: Vec<AcqFut<'_>> = Vec::new();
        for i in 0..12 {
            let mut f: AcqFut<'_> = Box::pin(gate.acquire(&users[i % 5], 3));
            match {
                let mut cx = Context::from_waker(futures::task::noop_waker_ref());
                f.as_mut().poll(&mut cx)
            } {
                Poll::Ready(p) => live.push(p),
                Poll::Pending => pending.push(f),
            }
        }
        assert_eq!(live.len(), 3, "僅 max 個立即授予");
        assert_eq!(snapshot(&gate).0, 3);
        for f in pending.iter_mut() {
            assert!(probe_pending(f), "其餘仍排隊");
        }
    }

    /// 飢餓優先：A 灌滿 → B、C 排隊；A 釋出一席後 B（零持有）先取得，
    /// A 不得在此刻擴展；再釋一席 → C。飽和態每使用者恰 1 線程。
    #[test]
    fn starving_users_win_over_widening() {
        let gate = FactoryGate::new();
        let a1 = grant(Box::pin(gate.acquire("A", 2)), "a1");
        let a2 = grant(Box::pin(gate.acquire("A", 2)), "a2 獨佔可展開到 max");
        let mut b: AcqFut<'_> = Box::pin(gate.acquire("B", 2));
        let mut c: AcqFut<'_> = Box::pin(gate.acquire("C", 2));
        assert!(probe_pending(&mut b) && probe_pending(&mut c), "B、C 排隊");

        drop(a1); // 釋出一席 → B 飢餓優先
        let b_p = grant(b, "B（零持有）先於 A 擴展取得");
        let (active, pu) = snapshot(&gate);
        assert_eq!(active, 2);
        assert_eq!(pu, vec![("A".into(), 1), ("B".into(), 1)], "A 不得在此刻擴展");

        assert!(probe_pending(&mut c), "C 仍排隊");
        drop(a2); // 再釋一席 → C（零持有）優先於任何擴展
        let c_p = grant(c, "C 飢餓優先取得");
        let (active, pu) = snapshot(&gate);
        assert_eq!(active, 2);
        assert_eq!(pu, vec![("B".into(), 1), ("C".into(), 1)], "飽和態每使用者恰 1 線程");
        drop((b_p, c_p));
    }

    /// 無競爭擴展合法：X 持 1 後擴展到 max；但 Y 一旦排隊，X 釋出的席位歸 Y。
    #[test]
    fn widening_only_when_no_starving() {
        let gate = FactoryGate::new();
        let x1 = grant(Box::pin(gate.acquire("X", 2)), "x1");
        let x2 = grant(Box::pin(gate.acquire("X", 2)), "x2 無競爭 → X 擴展");
        let mut y: AcqFut<'_> = Box::pin(gate.acquire("Y", 2));
        assert!(probe_pending(&mut y), "Y 排隊");
        drop(x1); // 席位歸 Y（飢餓優先），X 保留已取得的擴展成果 x2
        let y_p = grant(y, "Y 取得");
        assert_eq!(snapshot(&gate).1, vec![("X".into(), 1), ("Y".into(), 1)]);
        drop((x2, y_p));
    }

    /// 請求端取消：未授予的等待者被清掃；已授予者隨 drop 釋出，名額不外洩。
    #[test]
    fn cancelled_waiters_are_swept() {
        let gate = FactoryGate::new();
        let a = grant(Box::pin(gate.acquire("A", 1)), "a");
        // 排隊的幽靈等待者（未授予即 drop——模擬連線斷開）。
        let mut ghost: AcqFut<'_> = Box::pin(gate.acquire("ghost", 1));
        assert!(probe_pending(&mut ghost), "幽靈已排隊");
        drop(ghost);
        drop(a); // 釋出 → 掃描清掉幽靈、不授予任何東西
        let (active, pu) = snapshot(&gate);
        assert_eq!(active, 0);
        assert!(pu.is_empty(), "名額與持有全數歸零：{pu:?}");
        let mut st = FactoryGate::lock(&gate.inner);
        FactoryGate::dispatch_locked(&mut st);
        assert_eq!(st.queue.len(), 0, "佇列不殘留");
        drop(st); // 先釋鎖——下一段 acquire 要再鎖同一把（std Mutex 不可重入）
        // 閘門仍可用。
        let z = grant(Box::pin(gate.acquire("Z", 1)), "閘門仍可用");
        drop(z);
    }
}
