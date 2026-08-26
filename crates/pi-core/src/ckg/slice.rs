use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
};

use crate::ckg::graph::{CodeGraph, Direction, FileRegion, SliceResult, SymbolId};

pub fn slice(
    graph: &CodeGraph,
    seeds: &[SymbolId],
    direction: Direction,
    max_depth: usize,
) -> anyhow::Result<SliceResult> {
    let mut visited = HashSet::new();
    let mut symbol_ids = Vec::new();
    let mut files = HashMap::<PathBuf, (usize, usize)>::new();
    let mut edges = Vec::new();

    let mut queue: VecDeque<(SymbolId, usize)> = seeds.iter().map(|id| (*id, 0)).collect();
    for id in seeds {
        visited.insert(*id);
    }

    while let Some((id, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }
        if let Some(symbol) = graph.get(id) {
            symbol_ids.push(id);
            let entry = files
                .entry(symbol.file.clone())
                .or_insert((usize::MAX, usize::MIN));
            entry.0 = entry.0.min(symbol.line_range.0);
            entry.1 = entry.1.max(symbol.line_range.1);
        }

        let matches = match direction {
            Direction::Upstream => graph
                .edges
                .iter()
                .filter(|e| e.to == id)
                .collect::<Vec<_>>(),
            Direction::Downstream => graph
                .edges
                .iter()
                .filter(|e| e.from == id)
                .collect::<Vec<_>>(),
            Direction::Both => graph
                .edges
                .iter()
                .filter(|e| e.from == id || e.to == id)
                .collect::<Vec<_>>(),
        };

        for edge in matches {
            edges.push(edge.clone());
            let next = match direction {
                Direction::Upstream => edge.from,
                Direction::Downstream => edge.to,
                Direction::Both => {
                    if edge.from == id {
                        edge.to
                    } else {
                        edge.from
                    }
                }
            };
            if visited.insert(next) {
                queue.push_back((next, depth + 1));
            }
        }
    }

    symbol_ids.sort();
    symbol_ids.dedup();
    edges.sort_by(|a, b| {
        (a.from, a.to, format!("{:?}", a.kind)).cmp(&(b.from, b.to, format!("{:?}", b.kind)))
    });
    edges.dedup();

    let mut symbols = Vec::new();
    for id in symbol_ids {
        if let Some(symbol) = graph.get(id) {
            symbols.push(symbol.clone());
        }
    }

    let mut file_regions = Vec::new();
    for (file, (start, end)) in files {
        if start == usize::MAX || end == usize::MIN {
            continue;
        }
        file_regions.push(FileRegion {
            file,
            start_line: start,
            end_line: end,
        });
    }
    file_regions.sort_by(|a, b| a.file.cmp(&b.file));

    Ok(SliceResult {
        symbols,
        files: file_regions,
        edges,
    })
}
