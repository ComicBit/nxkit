//! `Dominance`: "A Simple, Fast Dominance Algorithm".
use super::Cfg;

impl Cfg {
    fn intersect(&self, mut b1: usize, mut b2: usize) -> Result<usize, String> {
        let m = &self.post_order_map;
        while b1 != b2 {
            while m[b1] < m[b2] {
                b1 = self.idom[b1].ok_or("no idom")?;
            }
            while m[b2] < m[b1] {
                b2 = self.idom[b2].ok_or("no idom")?;
            }
        }
        Ok(b1)
    }

    pub fn find_dominators(&mut self) -> Result<(), String> {
        if self.blocks.is_empty() {
            return Ok(());
        }
        self.idom[0] = Some(0);
        loop {
            let mut modified = false;
            for k in (0..self.post_order.len().saturating_sub(1)).rev() {
                let block = self.post_order[k];
                let mut new_idom: Option<usize> = None;
                for &p in &self.blocks[block].preds {
                    if self.idom[p].is_some() {
                        new_idom = Some(match new_idom {
                            Some(n) => self.intersect(p, n)?,
                            None => p,
                        });
                    }
                }
                if self.idom[block] != new_idom {
                    self.idom[block] = new_idom;
                    modified = true;
                }
            }
            if !modified {
                return Ok(());
            }
        }
    }

    pub fn find_dominance_frontiers(&mut self) -> Result<(), String> {
        for b in 0..self.blocks.len() {
            if self.blocks[b].preds.len() < 2 {
                continue;
            }
            for k in 0..self.blocks[b].preds.len() {
                let mut cur = self.blocks[b].preds[k];
                while Some(cur) != self.idom[b] {
                    self.frontiers[cur].add(b);
                    cur = self.idom[cur].ok_or("no idom")?;
                }
            }
        }
        Ok(())
    }
}
