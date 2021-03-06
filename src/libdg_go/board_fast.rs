// Copyright 2019 Karl Sundequist Blomdahl <karl.sundequist.blomdahl@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use color::Color;
use point::Point;
use point_state::Vertex;
use iter::{AdjacentIter, ChainIter, ValidIter, IsPartOf, NextLink};
use zobrist;

use std::ops::{Index, IndexMut};

/// Representation of a set of strongly connected vertices of the same color.
pub struct Block {
    board: *const BoardFast,
    starting_point: Point
}

impl<'a> Block {
    fn new(board: &BoardFast, starting_point: Point) -> Block {
        Block { board, starting_point }
    }
}

impl<'a> IntoIterator for Block {
    type Item = Point;
    type IntoIter = ChainIter<*const BoardFast>;

    fn into_iter(self) -> Self::IntoIter {
        ChainIter::new(self.starting_point, self.board)
    }
}

/// Minimal representation of a go board that implements all rules (except super-ko).
#[derive(Clone)]
pub struct BoardFast {
    /// Packed bit structure that contains the following fields. It has been padded
    /// with additional elements at the end that are used instead of out-of-bounds
    /// checks.
    ///
    /// - `color` - 2 bits
    /// - `next_vertex` - 10 bits
    /// - `head_vertex` - 10 bits
    /// - `num_liberties` - 9 bits
    /// - `visited` - 1 bit
    ///
    vertices: [u32; Point::MAX],
}

impl Index<Point> for BoardFast {
    type Output = u32;

    fn index(&self, index: Point) -> &Self::Output {
        &self.vertices[index]
    }
}

impl IndexMut<Point> for BoardFast {
    fn index_mut(&mut self, index: Point) -> &mut Self::Output {
        &mut self.vertices[index]
    }
}

impl IsPartOf for BoardFast {
    fn is_part_of(&self, point: Point) -> bool {
        let index = point.to_i();

        index < self.vertices.len() && self[point].is_valid()
    }
}

impl IsPartOf for &BoardFast {
    fn is_part_of(&self, point: Point) -> bool {
        (*self).is_part_of(point)
    }
}

impl NextLink for *const BoardFast {
    fn next_link(&self, point: Point) -> Point {
        unsafe { (**self)[point].next_point() }
    }
}

impl BoardFast {
    /// Returns an empty board.
    pub fn new() -> BoardFast {
        let mut board = BoardFast {
            vertices: [u32::invalid(); Point::MAX],
        };

        for point in Point::all() {
            board[point] = u32::empty();
        }

        board
    }

    /// Returns an iterator over all valid vertices that are adjacent to the
    /// given point.
    ///
    /// # Arguments
    ///
    /// * `at_point` -
    ///
    pub fn adjacent_to(&self, at_point: Point) -> ValidIter<AdjacentIter, &BoardFast> {
        ValidIter::new(
            AdjacentIter::new(at_point),
            &self
        )
    }

    /// Returns the block, set of strongly connected vertices of the same
    /// color, at the given vertex.
    ///
    /// # Arguments
    ///
    /// * `at_point` -
    ///
    pub fn block_at(&self, at_point: Point) -> Block {
        Block::new(self, at_point)
    }

    /// Returns whether the given liberties of the given group (as counted by
    /// the given counter). It will stop counting after `n` liberties has been
    /// found.
    /// 
    /// # Arguments
    /// 
    /// * `at_point` - the index of a vertex in the group
    /// 
    pub fn get_n_liberty(&self, at_point: Point) -> usize {
        let head = self[at_point].head_point();

        self[head].num_liberties()
    }

    /// Returns one of the liberties to the given block.
    /// 
    /// # Arguments
    /// 
    /// * `at_point` -
    /// 
    pub fn get_a_liberty(&self, at_point: Point) -> Option<Point> {
        for current in self.block_at(at_point) {
            for other_point in self.adjacent_to(current) {
                if self[other_point].color() == None {
                    return Some(other_point);
                }
            }
        }

        None
    }

    /// Returns whether the given group has at least `n` liberties, using the
    /// given counter to do so.
    /// 
    /// # Arguments
    /// 
    /// * `at_point` - the index of a vertex in the group
    /// * `n` - the maximum number of liberties to count
    /// 
    #[inline]
    pub fn has_n_liberty(&self, at_point: Point, n: usize) -> bool {
        let head = self[at_point].head_point();

        self[head].num_liberties() >= n
    }

    /// Returns whether the given move is valid according to the
    /// Tromp-Taylor rules.
    ///
    /// # Arguments
    ///
    /// * `color` - the color of the move
    /// * `at_point` - the HW index of the move
    ///
    pub fn is_valid(&self, color: Color, at_point: Point) -> bool {
        debug_assert!(self.is_part_of(at_point));

        self[at_point].color() == None && {
            let current = Some(color);

            for other_point in self.adjacent_to(at_point) {
                let value = self[other_point].color();

                // check for direct liberties
                if value == None {
                    return true;
                }

                // check for the following two conditions simplified into one case:
                //
                // 1. If a neighbour is friendly then we are fine if it has at
                //    least two liberties.
                // 2. If a neighbour is unfriendly then we are fine if it has less
                //    than two liberties (i.e. one).
                if (value == current) == self.has_n_liberty(other_point, 2) {
                    return true;
                }
            }

            false  // move is suicide :'(
        }
    }

    /// Returns if the given `liberty` is a liberty of one of the points
    /// that are part of the given `block_at`.
    /// 
    /// # Arguments
    /// 
    /// * `liberty` -
    /// * `block_at` - 
    /// 
    fn is_liberty_of(&self, liberty: Point, block_at: Point) -> bool {
        debug_assert_eq!(block_at, self[block_at].head_point());

        let block_color = self[block_at].color();

        self.adjacent_to(liberty).any(|adj_point| {
            let is_same_color = self[adj_point].color() == block_color;
            let is_same_block = self[adj_point].head_point() == block_at;

            is_same_color && is_same_block
        })
    }

    /// Connects the chains of the two vertices into one chain. This method
    /// should not be called with the same group twice as that will result
    /// in a corrupted chain.
    ///
    /// # Arguments
    ///
    /// * `next_vertex` - the array containing the next vertices
    /// * `one` - the first chain to connect
    /// * `two` - the second chain to connect
    ///
    #[inline]
    fn join_blocks(&mut self, one: Point, two: Point) {
        let head_one = self[one].head_point();
        let head_two = self[two].head_point();

        if head_one == head_two {
            return
        }

        // remove the liberty that we just filled by connecting these two
        // blocks.
        self[head_two].sub_liberties(1);

        // make each vertex in the first block part of the second block, and
        // calculate the number of additional liberties that the second
        // block gained.
        let mut already_added = [false; Point::MAX];
        let mut num_additional_liberties = 0;

        for point in self.block_at(one) {
            for adj_point in self.adjacent_to(point) {
                let is_empty = self[adj_point].color() == None;
                let is_new = !already_added[adj_point];

                if is_empty && is_new && !self.is_liberty_of(adj_point, head_two) {
                    already_added[adj_point] = true;
                    num_additional_liberties += 1;
                }
            }
        }

        for point in self.block_at(one) {
            self[point].set_head_point(head_two);
        }

        self[head_two].add_liberties(num_additional_liberties);

        // re-connect the two lists so if we have two chains `A` and `B`:
        //
        //   A:  a -> b -> c -> a
        //   B:  1 -> 2 -> 3 -> 1
        //
        // then the final new chain will be:
        //
        //   a -> 2 -> 3 -> 1 -> b -> c -> a
        //
        let one_prev = self[one].next_point();
        let two_prev = self[two].next_point();

        self[two].set_next_point(one_prev);
        self[one].set_next_point(two_prev);
    }

    /// Change the liberty count of each unique adjacent block to the given
    /// `starting_point` by one.
    /// 
    /// # Arguments
    /// 
    /// * `starting_point` - 
    /// * `delta` - 
    /// 
    fn incr_adjacent_liberties(&mut self, starting_point: Point) {
        let mut already_changed = [Point::default(); 4];
        let head = self[starting_point].head_point();

        for (i, adj_point) in AdjacentIter::new(starting_point).enumerate() {
            let adj_state = self[adj_point];

            if adj_state.color() != None {
                let adj_head = adj_state.head_point();
                let is_different_block = adj_head != head;
                let is_already_changed = already_changed.contains(&adj_head);

                if is_different_block && !is_already_changed {
                    already_changed[i] = adj_head;
                    self[adj_head].add_liberties(1);
                }
            }
        }
    }

    /// Returns the zobrist hash adjustment that would need to be done if the
    /// group at the given index was capture and was of the given color.
    ///
    /// # Arguments
    ///
    /// * `color` - the color of the group to capture
    /// * `at_point` - the index of a stone in the group
    ///
    #[inline]
    pub fn capture_if(&self, color: Color, at_point: Point) -> u64 {
        let mut adjust = 0;

        for current in self.block_at(at_point) {
            adjust ^= zobrist::TABLE[color as usize][current];
        }

        adjust
    }

    /// Remove all stones strongly connected to the given index from the
    /// board. It returns the necessary adjustment to the zobrist hash.
    ///
    /// # Arguments
    ///
    /// * `color` - the color of the group to capture
    /// * `at_point` - the index of a stone in the group to capture
    ///
    #[inline]
    pub fn capture(&mut self, color: Color, at_point: Point) -> u64 {
        let mut hash = 0;

        for other_index in self.block_at(at_point) {
            hash ^= zobrist::TABLE[color as usize][other_index];
            self[other_index].set_color(None);
            self.incr_adjacent_liberties(other_index);
        }

        hash
    }

    /// Returns the zobrist hash adjustments that are would be made if a stone
    /// of the given color was played on the given vertex.
    ///
    /// # Arguments
    ///
    /// * `color` - the color of the move
    /// * `at_point` - the HW index of the move
    ///
    #[inline]
    pub fn place_if(&self, color: Color, at_point: Point) -> u64 {
        let opponent = color.opposite();
        let mut seen_blocks = [Point::default(); 4];
        let mut adjust = zobrist::TABLE[color as usize][at_point];

        for (i, other_point) in self.adjacent_to(at_point).enumerate() {
            let head = self[other_point].head_point();

            if self[head].color() == Some(opponent) && !self.has_n_liberty(head, 2) {
                if !seen_blocks.contains(&head) {
                    seen_blocks[i] = head;
                    adjust ^= self.capture_if(opponent, head);
                }
            }
        }

        adjust
    }

    /// Place a some of the given `color` at the given `index` on this board. This function
    /// assume that the given move is valid.
    ///
    /// # Arguments
    ///
    /// * `color` -
    /// * `at_point` -
    ///
    #[inline]
    pub fn place(&mut self, color: Color, at_point: Point) -> u64 {
        // place the stone on the board regardless of whether it is legal
        // or not.
        let num_immediate_liberties = self
            .adjacent_to(at_point)
            .filter(|&adj_point| self[adj_point].color() == None)
            .count();

        self[at_point].set_color(Some(color));
        self[at_point].set_next_point(at_point);
        self[at_point].set_head_point(at_point);
        self[at_point].set_liberties(num_immediate_liberties);
        self[at_point].set_visited(true);

        // connect this stone to any neighbouring groups, and clear the
        // opponents color
        let mut hash = zobrist::TABLE[color as usize][at_point];
        let mut seen_blocks = [Point::default(); 4];
        let opponent = color.opposite();

        for (i, other_point) in AdjacentIter::new(at_point).enumerate() {
            let value = self[other_point].color();

            if value == Some(color) {
                self.join_blocks(at_point, other_point);
            } else if value == Some(opponent) {
                let head = self[other_point].head_point();

                if !seen_blocks.contains(&head) {
                    self[head].sub_liberties(1);
                    seen_blocks[i] = head;

                    if !self.has_n_liberty(head, 1) {
                        hash ^= self.capture(opponent, head);
                    }
                }
            }
        }

        hash
    }
}
