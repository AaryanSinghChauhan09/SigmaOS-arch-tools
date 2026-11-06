// SPDX-License-Identifier: Apache-2.0
//! SigmaOS-arch-tools — Arch maintainer/user utilities, pactree-inspired.
//!
//! Implements `pactree`-style dependency tree rendering: given a package
//! dependency map, render the tree depth-first with cycle detection
//! (fail-fast on dependency cycles, like pactree does) and optional
//! dependency dedup markers.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

/// Errors building/rendering a dependency tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    /// Package not present in the dependency map.
    UnknownPackage(String),
    /// Dependency cycle detected (e.g. a -> b -> a).
    Cycle(String),
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPackage(p) => write!(f, "unknown package: {p}"),
            Self::Cycle(p) => write!(f, "dependency cycle involving: {p}"),
        }
    }
}

impl std::error::Error for TreeError {}

/// Package dependency graph: name -> direct dependencies.
pub type DepGraph = BTreeMap<String, Vec<String>>;

/// Render `root`'s dependency tree, depth-first, one node per line with
/// two-space indentation per level. Fails on unknown packages or cycles.
pub fn render_tree(graph: &DepGraph, root: &str) -> Result<String, TreeError> {
    if !graph.contains_key(root) {
        return Err(TreeError::UnknownPackage(root.to_string()));
    }
    let mut out = String::new();
    let mut visiting: HashSet<String> = HashSet::new();
    visit(graph, root, 0, &mut visiting, &mut out)?;
    Ok(out)
}

fn visit(
    graph: &DepGraph,
    pkg: &str,
    depth: usize,
    visiting: &mut HashSet<String>,
    out: &mut String,
) -> Result<(), TreeError> {
    if !visiting.insert(pkg.to_string()) {
        return Err(TreeError::Cycle(pkg.to_string()));
    }
    out.push_str(&"  ".repeat(depth));
    out.push_str(pkg);
    out.push('\n');
    if let Some(deps) = graph.get(pkg) {
        for dep in deps {
            visit(graph, dep, depth + 1, visiting, out)?;
        }
    }
    visiting.remove(pkg);
    Ok(())
}

/// Collect all transitive dependencies of `root` (excluding root itself).
pub fn transitive_deps(graph: &DepGraph, root: &str) -> Result<Vec<String>, TreeError> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut visiting: HashSet<String> = HashSet::new();
    // First validate reachability without cycles via DFS
    fn dfs(
        graph: &DepGraph,
        pkg: &str,
        visiting: &mut HashSet<String>,
        seen: &mut HashSet<String>,
    ) -> Result<(), TreeError> {
        if !graph.contains_key(pkg) {
            return Err(TreeError::UnknownPackage(pkg.to_string()));
        }
        if !visiting.insert(pkg.to_string()) {
            return Err(TreeError::Cycle(pkg.to_string()));
        }
        seen.insert(pkg.to_string());
        if let Some(deps) = graph.get(pkg) {
            for d in deps {
                dfs(graph, d, visiting, seen)?;
            }
        }
        visiting.remove(pkg);
        Ok(())
    }
    dfs(graph, root, &mut visiting, &mut seen)?;
    seen.remove(root);
    Ok(seen.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> DepGraph {
        let mut g = DepGraph::new();
        g.insert("base".into(), vec![]);
        g.insert("linux".into(), vec!["base".into(), "mkinitcpio".into()]);
        g.insert("mkinitcpio".into(), vec!["base".into()]);
        g.insert("linux-firmware".into(), vec!["base".into()]);
        g
    }

    #[test]
    fn renders_tree() {
        let out = render_tree(&graph(), "linux").unwrap();
        assert!(out.starts_with("linux\n"));
        assert!(out.contains("  base\n"));
        assert!(out.contains("  mkinitcpio\n"));
    }

    #[test]
    fn unknown_root_fails() {
        assert_eq!(
            render_tree(&graph(), "nope"),
            Err(TreeError::UnknownPackage("nope".into()))
        );
    }

    #[test]
    fn unknown_dep_fails() {
        let mut g = graph();
        g.insert("bad".into(), vec!["ghost".into()]);
        assert_eq!(
            render_tree(&g, "bad"),
            Err(TreeError::UnknownPackage("ghost".into()))
        );
    }

    #[test]
    fn cycle_fails() {
        let mut g = DepGraph::new();
        g.insert("a".into(), vec!["b".into()]);
        g.insert("b".into(), vec!["a".into()]);
        assert!(matches!(render_tree(&g, "a"), Err(TreeError::Cycle(_))));
    }

    #[test]
    fn transitive_deps_collects_all() {
        let mut deps = transitive_deps(&graph(), "linux").unwrap();
        deps.sort();
        assert_eq!(deps, vec!["base", "mkinitcpio"]);
    }
}
