use std::{default::Default, mem};

// Aliased so we can swap them to BTree versions easily.
pub(crate) type Set<T> = std::collections::HashSet<T>;
pub(crate) type Map<K, V> = std::collections::HashMap<K, V>;
pub(crate) type BlockId = u64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Block {
	pub(crate) id: BlockId,
	pub(crate) from: Set<BlockId>,
	pub(crate) to: Set<BlockId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph(pub(crate) Map<u64, Block>);

impl Graph {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	/// Insert a node connection. Returns true if the connection
	/// is new.
	pub fn update(&mut self, from: u64, to: u64) -> bool {
		let mut modified = false;
		self.0
			.entry(from)
			.and_modify(|node| {
				modified |= node.to.insert(to);
			})
			.or_insert_with(|| {
				modified = true;
				Block {
					id: from,
					to: [to].into_iter().collect(),
					from: Default::default(),
				}
			});
		self.0
			.entry(to)
			.and_modify(|node| {
				modified |= node.from.insert(from);
			})
			.or_insert_with(|| {
				modified = true;
				Block {
					id: to,
					to: Default::default(),
					from: [from].into_iter().collect(),
				}
			});

		modified
	}

	/// Compresses graph by merging every node pair that always go
	/// from one to the other
	pub fn compress(&mut self) {
		let m = &mut self.0;

		// Visit every node in arbitrary order.
		// We have to check (a, b) AND (b, a) seperately
		//  because we have a directed *cyclic* graph.
		// Following a depth-first order would just require
		//  a visited collection for no benefit.
		// We have to traverse the graph twice anyway because
		//  of the borrow checker.
		// The first value is always the smallest and the
		//  merged node will take the id of the smallest of
		//  the parents.
		let to_merge = m
			.values()
			.filter(|l| l.from.len() == 1)
			.map(|l| (l, &m[l.from.iter().next().unwrap()]))
			.filter(|(_, r)| r.to.len() == 1)
			.map(|(l, r)| (l.id, r.id))
			.collect::<Set<_>>();

		let mut translation_map = Map::new();
		fn translate(key: u64, map: &Map<u64, u64>) -> u64 {
			match map.get(&key) {
				Some(k) => translate(*k, map),
				None => key,
			}
		}

		for (mut l, mut r) in to_merge.into_iter() {
			l = translate(l, &translation_map);
			r = translate(r, &translation_map);

			let x = l.min(r);
			let y = l.max(r);
			self.merge_nodes(x, y);

			translation_map.insert(l, x);
			translation_map.insert(r, x);
			translation_map.insert(y, x);
			translation_map.remove(&x);
		}
	}

	fn are_loop(&self, l: u64, r: u64) -> bool {
		if l == r {
			return true;
		}
		let m = &self.0;
		m[&l].from.contains(&r) && m[&r].from.contains(&l)
	}

	pub fn merge_nodes(&mut self, l: u64, r: u64) {
		if l > r {
			self.merge_nodes(r, l);
			return;
		}
		if l == r {
			return;
		}

		assert!(self.0.contains_key(&l));
		assert!(self.0.contains_key(&r));

		let are_loop = self.are_loop(l, r);
		let map = &mut self.0;

		// Take the union of both nodes' input and then remove
		// the nodes themselves
		let to_r = mem::take(&mut map.get_mut(&r).unwrap().to);
		let from_r = mem::take(&mut map.get_mut(&r).unwrap().from);

		let l_ref = map.get_mut(&l).unwrap();

		for node in to_r.into_iter().filter(|&x| x != l && x != r) {
			l_ref.to.insert(node);
		}
		l_ref.to.remove(&l);
		l_ref.to.remove(&r);

		for node in from_r.into_iter().filter(|&x| x != l && x != r) {
			l_ref.from.insert(node);
		}
		l_ref.from.remove(&l);
		l_ref.from.remove(&r);

		// Restore loop if they were a loop beforehand
		if are_loop {
			l_ref.from.insert(l);
			l_ref.to.insert(l);
		}

		// Remove the right node from the graph
		map.remove(&r);

		// And fix any pointers to the right node so that they
		// point to the left node
		for node in map.values_mut() {
			if node.from.remove(&r) {
				node.from.insert(l);
			}
			if node.to.remove(&r) {
				node.to.insert(l);
			}
		}
	}

	/// Split `node` into two nodes, with the new node using the
	/// requested id if it's not already in use. Returns the id of
	/// the new node
	pub fn split_node(&mut self, node: u64, requested_id: u64) -> u64 {
		if self.0.contains_key(&requested_id) {
			return self.split_node(node, requested_id + 1);
		}

		// Swap the existing outgoing set with a set pointing
		// solely to the new node
		let mut to = [requested_id].into_iter().collect::<Set<_>>();
		mem::swap(&mut self.0.get_mut(&node).unwrap().to, &mut to);

		// Fix incoming sets of connected nodes
		for connection_id in to.iter() {
			let connection = self.0.get_mut(connection_id).unwrap();
			assert!(connection.from.remove(&node));
			connection.from.insert(requested_id);
		}

		// And create the new node
		let from = [node].into_iter().collect::<Set<_>>();
		let block = Block {
			id: requested_id,
			to,
			from,
		};
		self.0.insert(requested_id, block);

		requested_id
	}

	/// Verify that all node pairs have matching to and from
	#[cfg(test)]
	fn verify(&self) {
		let m = &self.0;

		for (k, v) in m.iter() {
			for out in v.from.iter() {
				assert!(
					m[out].to.contains(k),
					"{out}.to contains {k}?\n{self:#?}"
				);
			}
		}

		for (k, v) in m.iter() {
			for to in v.to.iter() {
				assert!(
					m[to].from.contains(k),
					"{to}.from contains {k}?\n{self:#?}"
				);
			}
		}
	}
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowGraph {
	graph: Graph,
	compressed_graph: Graph,
	last: BlockId,
}

impl<const N: usize, const M: usize> From<(BlockId, [BlockId; N], [BlockId; M])> for Block {
	fn from((id, f, t): (BlockId, [BlockId; N], [BlockId; M])) -> Self {
		Block {
			id,
			from: f.into_iter().collect(),
			to: t.into_iter().collect(),
		}
	}
}

#[cfg(test)]
mod test {
	use crate::graph::*;

	/// 0 → 1 → 2
	#[test]
	fn straight_line() {
		let mut graph = Graph(
			[
				(0, (0, [], [1]).into()),
				(1, (1, [0], [2]).into()),
				(2, (2, [1], []).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph([(0, (0, [], []).into())].into_iter().collect());
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 2 → 1 → 0
	#[test]
	fn straight_line_rev() {
		let mut graph = Graph(
			[
				(0, (0, [1], []).into()),
				(1, (1, [2], [0]).into()),
				(2, (2, [], [1]).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph([(0, (0, [], []).into())].into_iter().collect());
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 0 → 1
	#[test]
	fn short_line() {
		let mut graph = Graph(
			[
				(0, (0, [], [1]).into()),
				(1, (1, [0], []).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph([(0, (0, [], []).into())].into_iter().collect());
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 1 → 0
	#[test]
	fn short_line_rev() {
		let mut graph = Graph(
			[(0, (0, [1], []).into()), (1, (1, [], [0]).into())]
				.into_iter()
				.collect(),
		);
		let expected = Graph([(0, (0, [], []).into())].into_iter().collect());
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	///   0
	///  ↙ ↘
	/// 1   2
	///  ↘ ↙
	///   3
	#[test]
	fn diamond() {
		let mut graph = Graph(
			[
				(0, (0, [], [1, 2]).into()),
				(1, (1, [0], [3]).into()),
				(2, (2, [0], [3]).into()),
				(3, (3, [1, 2], []).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = graph.clone();
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	///   3
	///  ↙ ↘
	/// 1   2
	///  ↘ ↙
	///   0
	#[test]
	fn diamond_rev() {
		let mut graph = Graph(
			[
				(0, (0, [1, 2], []).into()),
				(1, (1, [3], [0]).into()),
				(2, (2, [3], [0]).into()),
				(3, (3, [], [1, 2]).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = graph.clone();
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 4 → 0
	/// ↑  ↙ ↘
	/// 5 1   2
	/// ↑  ↘ ↙
	/// 6   3
	#[test]
	fn diamond_on_stick() {
		let mut graph = Graph(
			[
				(0, (0, [4], [1, 2]).into()),
				(1, (1, [0], [3]).into()),
				(2, (2, [0], [3]).into()),
				(3, (3, [1, 2], []).into()),
				(4, (4, [5], [0]).into()),
				(5, (5, [6], [4]).into()),
				(6, (6, [], [5]).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph(
			[
				(0, (0, [], [1, 2]).into()),
				(1, (1, [0], [3]).into()),
				(2, (2, [0], [3]).into()),
				(3, (3, [1, 2], []).into()),
			]
			.into_iter()
			.collect(),
		);
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 6 → 3
	/// ↑  ↙ ↘
	/// 5 1   2
	/// ↑  ↘ ↙
	/// 4   0
	#[test]
	fn diamond_on_stick_rev() {
		let mut graph = Graph(
			[
				(0, (0, [1, 2], []).into()),
				(1, (1, [3], [0]).into()),
				(2, (2, [3], [0]).into()),
				(3, (3, [6], [1, 2]).into()),
				(4, (4, [], [5]).into()),
				(5, (5, [4], [6]).into()),
				(6, (6, [5], [3]).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph(
			[
				(0, (0, [1, 2], []).into()),
				(1, (1, [3], [0]).into()),
				(2, (2, [3], [0]).into()),
				(3, (3, [], [1, 2]).into()),
			]
			.into_iter()
			.collect(),
		);
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 0   1
	///  ↘ ↙
	///   2
	///   ↓
	///   3
	///  ↙ ↘
	/// 4   5
	#[test]
	fn cross() {
		let mut graph = Graph(
			[
				(0, (0, [], [2]).into()),
				(1, (1, [], [2]).into()),
				(2, (2, [0, 1], [3]).into()),
				(3, (3, [2], [4, 5]).into()),
				(4, (4, [3], []).into()),
				(5, (5, [3], []).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph(
			[
				(0, (0, [], [2]).into()),
				(1, (1, [], [2]).into()),
				(2, (2, [0, 1], [4, 5]).into()),
				(4, (4, [2], []).into()),
				(5, (5, [2], []).into()),
			]
			.into_iter()
			.collect(),
		);
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 4   5
	///  ↘ ↙
	///   3
	///   ↓
	///   2
	///  ↙ ↘
	/// 0   1
	#[test]
	fn cross_rev() {
		let mut graph = Graph(
			[
				(0, (0, [2], []).into()),
				(1, (1, [2], []).into()),
				(2, (2, [3], [0, 1]).into()),
				(3, (3, [4, 5], [2]).into()),
				(4, (4, [], [3]).into()),
				(5, (5, [], [3]).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph(
			[
				(0, (0, [2], []).into()),
				(1, (1, [2], []).into()),
				(2, (2, [4, 5], [0, 1]).into()),
				(4, (4, [], [2]).into()),
				(5, (5, [], [2]).into()),
			]
			.into_iter()
			.collect(),
		);
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 0   1
	///  ↘ ↙
	///   2
	///   ↓
	///   3
	///  ↙ ↘
	/// 4   5
	#[test]
	fn cross_split() {
		let mut graph = Graph(
			[
				(0, (0, [], [2]).into()),
				(1, (1, [], [2]).into()),
				(2, (2, [0, 1], [4, 5]).into()),
				(4, (4, [2], []).into()),
				(5, (5, [2], []).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph(
			[
				(0, (0, [], [2]).into()),
				(1, (1, [], [2]).into()),
				(2, (2, [0, 1], [3]).into()),
				(3, (3, [2], [4, 5]).into()),
				(4, (4, [3], []).into()),
				(5, (5, [3], []).into()),
			]
			.into_iter()
			.collect(),
		);
		graph.verify();
		expected.verify();
		let node = graph.split_node(2, 3);
		graph.verify();
		assert_eq!(graph, expected);
		assert_eq!(node, 3);
	}

	///   0
	///  ↙ ↖
	/// 1   3
	///  ↘ ↗
	///   2
	#[test]
	fn cycle() {
		let mut graph = Graph(
			[
				(0, (0, [3], [1]).into()),
				(1, (1, [0], [2]).into()),
				(2, (2, [1], [3]).into()),
				(3, (3, [2], [0]).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph([(0, (0, [0], [0]).into())].into_iter().collect());
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	///   0
	///  ↙ ↖
	/// 3   1
	///  ↘ ↗
	///   2
	#[test]
	fn cycle_rev() {
		let mut graph = Graph(
			[
				(0, (0, [1], [3]).into()),
				(1, (1, [2], [0]).into()),
				(2, (2, [3], [1]).into()),
				(3, (3, [0], [2]).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph([(0, (0, [0], [0]).into())].into_iter().collect());
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}

	/// 0   1
	/// ↓   ↓
	/// 2   3
	///  ↘ ↙
	///   4
	#[test]
	fn v() {
		let mut graph = Graph(
			[
				(0, (0, [], [2]).into()),
				(1, (1, [], [3]).into()),
				(2, (2, [0], [4]).into()),
				(3, (3, [1], [4]).into()),
				(4, (4, [2, 3], []).into()),
			]
			.into_iter()
			.collect(),
		);
		let expected = Graph(
			[
				(0, (0, [], [4]).into()),
				(1, (1, [], [4]).into()),
				(4, (4, [0, 1], []).into()),
			]
			.into_iter()
			.collect(),
		);
		graph.verify();
		expected.verify();
		graph.compress();
		graph.verify();
		assert_eq!(graph, expected);
	}
}