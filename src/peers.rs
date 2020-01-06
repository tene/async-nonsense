use im::hashmap::HashMap;

use crate::AgentId;

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
struct Distance(usize);

#[derive(Debug, Default)]
pub struct Peers {
    cache: HashMap<usize, HashMap<AgentId, Distance>>,
    best: HashMap<AgentId, (usize, Distance)>,
    links: HashMap<AgentId, HashMap<usize, Distance>>,
}

impl Peers {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn best_link(&self, id: &AgentId) -> Option<usize> {
        self.best.get(id).map(|b| b.0)
    }
    pub fn peer_table<'a>(&'a self) -> impl Iterator<Item = (AgentId, usize)> + 'a {
        self.best
            .iter()
            .cloned()
            .map(|(k, (_, Distance(v)))| (k, v))
    }
    pub fn peer(&mut self, link: usize, id: AgentId) {
        self.links
            .entry(id.clone())
            .or_default()
            .insert(link, Distance(0));
        self.best.insert(id.clone(), (link, Distance(0)));
        self.cache.entry(link).or_default().insert(id, Distance(0));
    }
    pub fn update(
        &mut self,
        link: usize,
        id: AgentId,
        peers: Vec<(AgentId, usize)>,
    ) -> (Vec<AgentId>, Vec<AgentId>) {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut cache: HashMap<AgentId, Distance> = peers
            .iter()
            .map(|(id, dist)| (id.clone(), Distance(dist + 1)))
            .collect();
        cache.insert(id, Distance(0));
        for (id, dist) in cache.iter() {
            match self.best.get(id) {
                Some((_, best)) => {
                    if dist < best {
                        self.best.insert(id.clone(), (link, *dist));
                    }
                }
                None => {
                    added.push(id.clone());
                    self.best.insert(id.clone(), (link, *dist));
                }
            }
            self.links
                .entry(id.clone())
                .or_default()
                .insert(link, *dist);
        }
        for id in self
            .cache
            .entry(link)
            .or_default()
            .clone()
            .relative_complement(cache.clone())
            .keys()
        {
            self.drop_link_helper(id, link).map(|id| removed.push(id));
        }
        self.cache.insert(link, cache);
        (added, removed)
    }
    fn drop_link_helper(&mut self, id: &AgentId, link: usize) -> Option<AgentId> {
        let links = &mut self.links[id];
        links.remove(&link);
        match links.iter().min_by_key(|p| p.1) {
            Some(best) => {
                self.best[id] = *best;
                None
            }
            None => {
                self.best.remove(id);
                Some(id.clone())
            }
        }
    }
    pub fn drop_link(&mut self, link: usize) -> Vec<AgentId> {
        self.cache
            .remove(&link)
            .expect("Tried to drop missing link")
            .keys()
            .filter_map(|id| self.drop_link_helper(id, link))
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use itertools::{assert_equal, sorted};

    #[test]
    fn basics() {
        let mut peers = Peers::new();
        let a = AgentId::new("mars");
        let a1 = AgentId::new("foo");
        let a2 = AgentId::new("foo2");
        let a3 = AgentId::new("foo3");
        let (new, lost) = peers.update(0, a.clone(), vec![(a1.clone(), 0), (a2.clone(), 0)]);
        assert_equal(sorted(new), sorted(vec![a.clone(), a1.clone(), a2.clone()]));
        assert_eq!(lost, vec![]);
        let (new, lost) = peers.update(0, a.clone(), vec![(a3.clone(), 0), (a2.clone(), 0)]);
        assert_equal(sorted(new), sorted(vec![a3.clone()]));
        assert_eq!(lost, vec![a1.clone()]);
    }
}
