use std::collections::HashMap;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct LogCluster {
    pub template: Vec<String>,
    pub id: usize,
    pub size: usize,
}

pub struct DrainParser {
    depth: usize,
    sim_threshold: f64,
    max_children: usize,
    root: Node,
    clusters: Vec<LogCluster>,
    next_id: usize,
    header_filter: Vec<Regex>,
}

enum Node {
    Internal(HashMap<String, Node>),
    Leaf(Vec<usize>), // Stores indices of clusters
}

impl DrainParser {
    pub fn new(depth: usize, sim_threshold: f64, max_children: usize) -> Self {
        Self {
            depth,
            sim_threshold,
            max_children,
            root: Node::Internal(HashMap::new()),
            clusters: Vec::new(),
            next_id: 0,
            header_filter: vec![
                Regex::new(r"\d+").unwrap(), // Digits
                Regex::new(r"(0x)[0-9a-fA-F]+").unwrap(), // Hex
            ],
        }
    }

    /// Pre-processes the log: removes noise and tokens to list
    fn preprocess(&self, content: &str) -> Vec<String> {
        content
            .split_whitespace()
            .map(|s| {
                let res = s.to_string();
                for re in &self.header_filter {
                    if re.is_match(&res) {
                        return "<*>".to_string();
                    }
                }
                res
            })
            .collect()
    }

    fn traverse_to_leaf(&mut self, tokens: &[String], log_len: usize) -> &mut Vec<usize> {
        let mut current_node = &mut self.root;

        current_node = match current_node {
            Node::Internal(children) => {
                children.entry(log_len.to_string()).or_insert(Node::Internal(HashMap::new()))
            }
            _ => unreachable!(),
        };

        for i in 0..(self.depth - 2).min(log_len) {
            let token = tokens[i].clone();
            let is_last = i == self.depth - 3;
            current_node = match current_node {
                Node::Internal(children) => {
                    let make = || if is_last { Node::Leaf(Vec::new()) } else { Node::Internal(HashMap::new()) };
                    if children.len() < self.max_children || children.contains_key(&token) {
                        children.entry(token).or_insert_with(make)
                    } else {
                        children.entry("<*>".to_string()).or_insert_with(make)
                    }
                }
                _ => unreachable!(),
            };
        }

        match current_node {
            Node::Leaf(indices) => indices,
            _ => panic!("Tree depth logic mismatch"),
        }
    }

    /// The core deterministic parsing logic
    pub fn parse(&mut self, content: &str) -> &LogCluster {
        let tokens = self.preprocess(content);
        let log_len = tokens.len();

        // Snapshot leaf indices — releases the mutable borrow of the tree
        let candidate_indices: Vec<usize> = self.traverse_to_leaf(&tokens, log_len).clone();

        // Search for best-matching cluster (can borrow self.clusters freely now)
        let best_match = candidate_indices.iter().copied()
            .filter_map(|idx| {
                let sim = self.calculate_similarity(&self.clusters[idx].template, &tokens);
                if sim >= self.sim_threshold { Some((idx, sim)) } else { None }
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if let Some((idx, _)) = best_match {
            self.update_template(idx, &tokens);
            &self.clusters[idx]
        } else {
            let new_idx = self.clusters.len();
            self.clusters.push(LogCluster { template: tokens.clone(), id: self.next_id, size: 1 });
            self.next_id += 1;
            self.traverse_to_leaf(&tokens, log_len).push(new_idx);
            &self.clusters[new_idx]
        }
    }

    fn calculate_similarity(&self, template: &[String], tokens: &[String]) -> f64 {
        if template.len() != tokens.len() { return 0.0; }
        let mut matches = 0;
        for (t1, t2) in template.iter().zip(tokens.iter()) {
            if t1 == t2 { matches += 1; }
        }
        matches as f64 / template.len() as f64
    }

    fn update_template(&mut self, idx: usize, tokens: &[String]) {
        let cluster = &mut self.clusters[idx];
        cluster.size += 1;
        for i in 0..cluster.template.len() {
            if cluster.template[i] != tokens[i] {
                cluster.template[i] = "<*>".to_string();
            }
        }
    }

    pub fn clusters(&self) -> &[LogCluster] {
        &self.clusters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> DrainParser {
        DrainParser::new(4, 0.5, 100)
    }

    fn template(c: &LogCluster) -> String {
        c.template.join(" ")
    }

    #[test]
    fn new_line_creates_cluster() {
        let mut p = parser();
        let c = p.parse("user login success");
        assert_eq!(c.id, 0);
        assert_eq!(c.size, 1);
        assert_eq!(template(c), "user login success");
    }

    #[test]
    fn identical_lines_merge() {
        let mut p = parser();
        let id = p.parse("connected to host").id;
        let c = p.parse("connected to host");
        assert_eq!(c.id, id);
        assert_eq!(c.size, 2);
    }

    #[test]
    fn differing_token_becomes_wildcard() {
        // depth=4 routes on tokens[0] and tokens[1]; put the variable token at
        // position [3] so both lines reach the same leaf before similarity search.
        let mut p = parser();
        p.parse("user logged in alice");
        let c = p.parse("user logged in bob");
        assert_eq!(template(c), "user logged in <*>");
        assert_eq!(c.size, 2);
    }

    #[test]
    fn numeric_token_preprocessed_to_wildcard() {
        let mut p = parser();
        p.parse("request took 120 ms");
        let c = p.parse("request took 95 ms");
        // Both numeric tokens are already replaced by preprocess, so template
        // should treat them as identical wildcards and merge.
        assert_eq!(template(c), "request took <*> ms");
        assert_eq!(c.size, 2);
    }

    #[test]
    fn hex_token_preprocessed_to_wildcard() {
        let mut p = parser();
        let c = p.parse("addr 0xdeadbeef allocated");
        assert_eq!(template(c), "addr <*> allocated");
    }

    #[test]
    fn different_lengths_produce_separate_clusters() {
        let mut p = parser();
        let id1 = p.parse("disk full").id;
        let id2 = p.parse("disk almost full now").id;
        assert_ne!(id1, id2);
        assert_eq!(p.clusters().len(), 2);
    }

    #[test]
    fn below_threshold_creates_new_cluster() {
        // sim_threshold = 0.9 — only 1/4 tokens match, well below threshold
        let mut p = DrainParser::new(4, 0.9, 100);
        p.parse("alpha beta gamma delta");
        let size = p.parse("alpha zzzz yyyy xxxx").size;
        assert_eq!(p.clusters().len(), 2, "dissimilar line should not merge");
        assert_eq!(size, 1);
    }

    #[test]
    fn multiple_variables_in_template() {
        let mut p = parser();
        p.parse("ERROR port 8080 host db1 failed");
        let c = p.parse("ERROR port 9090 host db2 failed");
        assert_eq!(template(c), "ERROR port <*> host <*> failed");
    }

    #[test]
    fn cluster_ids_are_monotone() {
        let mut p = parser();
        p.parse("alpha bravo charlie delta");
        p.parse("one two three four");
        p.parse("foo bar baz qux");
        let ids: Vec<usize> = p.clusters().iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn size_tracks_merged_count() {
        let mut p = parser();
        for i in 0..5usize {
            p.parse(&format!("worker {} started", i));
        }
        assert_eq!(p.clusters().len(), 1);
        assert_eq!(p.clusters()[0].size, 5);
    }
}
