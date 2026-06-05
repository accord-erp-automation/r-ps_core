use std::sync::{Arc, Mutex};

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrintActivitySnapshot {
    pub busy: bool,
    pub status: String,
    pub label: String,
    pub detail: String,
    pub epc: String,
    pub item_code: String,
    pub item_name: String,
    pub printer: String,
}

impl PrintActivitySnapshot {
    pub fn idle() -> Self {
        Self {
            busy: false,
            status: "idle".to_string(),
            label: "Bo'sh".to_string(),
            detail: String::new(),
            epc: String::new(),
            item_code: String::new(),
            item_name: String::new(),
            printer: String::new(),
        }
    }

    pub fn printing(epc: &str, item_code: &str, item_name: &str, printer: &str) -> Self {
        Self {
            busy: true,
            status: "printing".to_string(),
            label: "Band".to_string(),
            detail: "Printer server boshqa mobile print so'rovi bilan band.".to_string(),
            epc: epc.to_string(),
            item_code: item_code.to_string(),
            item_name: item_name.to_string(),
            printer: printer.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PrintActivityState {
    inner: Arc<Mutex<PrintActivitySnapshot>>,
}

impl Default for PrintActivityState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PrintActivitySnapshot::idle())),
        }
    }
}

impl PrintActivityState {
    pub fn snapshot(&self) -> PrintActivitySnapshot {
        self.inner
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| PrintActivitySnapshot::idle())
    }

    pub fn try_start(
        &self,
        epc: &str,
        item_code: &str,
        item_name: &str,
        printer: &str,
    ) -> Result<PrintActivityGuard, PrintActivitySnapshot> {
        let mut snapshot = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if snapshot.busy {
            return Err(snapshot.clone());
        }
        *snapshot = PrintActivitySnapshot::printing(epc, item_code, item_name, printer);
        Ok(PrintActivityGuard {
            state: self.clone(),
        })
    }

    fn clear(&self) {
        let mut snapshot = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *snapshot = PrintActivitySnapshot::idle();
    }
}

#[derive(Debug)]
pub struct PrintActivityGuard {
    state: PrintActivityState,
}

impl Drop for PrintActivityGuard {
    fn drop(&mut self) {
        self.state.clear();
    }
}
