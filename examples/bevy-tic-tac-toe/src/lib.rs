//! Game rules shared by the Bevy interface and exhaustive opponent checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mark {
    X,
    O,
}

impl Mark {
    pub fn other(self) -> Self {
        match self {
            Self::X => Self::O,
            Self::O => Self::X,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::O => "O",
        }
    }
}

pub const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Playing,
    Won(Mark, [usize; 3]),
    Draw,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveError {
    NotYourTurn,
    OutsideBoard,
    Occupied,
    Finished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Board {
    cells: [Option<Mark>; 9],
    turn: Mark,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            cells: [None; 9],
            turn: Mark::X,
        }
    }
}

impl Board {
    pub fn cells(&self) -> &[Option<Mark>; 9] {
        &self.cells
    }
    pub fn turn(&self) -> Mark {
        self.turn
    }
    pub fn outcome(&self) -> Outcome {
        for line in LINES {
            if let Some(mark) = self.cells[line[0]]
                && line.iter().all(|&cell| self.cells[cell] == Some(mark))
            {
                return Outcome::Won(mark, line);
            }
        }
        if self.cells.iter().all(Option::is_some) {
            Outcome::Draw
        } else {
            Outcome::Playing
        }
    }
    pub fn play(&mut self, cell: usize) -> Result<(), MoveError> {
        if cell >= 9 {
            return Err(MoveError::OutsideBoard);
        }
        if self.outcome() != Outcome::Playing {
            return Err(MoveError::Finished);
        }
        if self.cells[cell].is_some() {
            return Err(MoveError::Occupied);
        }
        self.cells[cell] = Some(self.turn);
        self.turn = self.turn.other();
        Ok(())
    }
    pub fn legal_moves(&self) -> impl Iterator<Item = usize> + '_ {
        (0..9).filter(|&i| self.outcome() == Outcome::Playing && self.cells[i].is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn played(moves: &[usize]) -> Board {
        let mut board = Board::default();
        for &cell in moves {
            board.play(cell).unwrap();
        }
        board
    }

    #[test]
    fn illegal_moves_leave_both_board_and_turn_unchanged() {
        let mut board = played(&[4]);
        let before = board.clone();
        assert_eq!(board.play(4), Err(MoveError::Occupied));
        assert_eq!(board.play(9), Err(MoveError::OutsideBoard));
        assert_eq!(board, before);
        assert_eq!(board.turn(), Mark::O);
    }

    #[test]
    fn recognizes_all_eight_winning_lines_for_both_players() {
        for line in LINES {
            for mark in [Mark::X, Mark::O] {
                let mut board = Board::default();
                for cell in line {
                    board.cells[cell] = Some(mark);
                }
                assert_eq!(board.outcome(), Outcome::Won(mark, line));
            }
        }
    }

    #[test]
    fn completed_games_reject_more_moves() {
        let mut board = played(&[0, 3, 1, 4, 2]);
        assert_eq!(board.outcome(), Outcome::Won(Mark::X, [0, 1, 2]));
        let before = board.clone();
        assert_eq!(board.play(8), Err(MoveError::Finished));
        assert_eq!(board, before);
        assert_eq!(board.legal_moves().count(), 0);
    }

    #[test]
    fn a_full_board_without_a_line_is_a_draw() {
        let board = played(&[0, 4, 8, 1, 7, 6, 2, 5, 3]);
        assert_eq!(board.outcome(), Outcome::Draw);
        assert_eq!(board.legal_moves().count(), 0);
    }

    #[test]
    fn a_win_on_the_ninth_move_is_not_a_draw() {
        let board = played(&[0, 1, 2, 3, 4, 5, 7, 6, 8]);
        assert_eq!(board.outcome(), Outcome::Won(Mark::X, [0, 4, 8]));
    }
}

/// Choose an optimal legal move. Stable ordering makes recordings reproducible.
pub fn best_move(board: &Board) -> Option<usize> {
    if board.outcome() != Outcome::Playing {
        return None;
    }
    let player = board.turn();
    let mut best = None;
    let mut best_score = i16::MIN;
    for cell in [4, 0, 2, 6, 8, 1, 3, 5, 7] {
        if board.cells[cell].is_some() {
            continue;
        }
        let mut next = board.clone();
        next.play(cell)
            .expect("an empty cell on a live board is legal");
        let score = minimax(&next, player, 1);
        if score > best_score {
            best_score = score;
            best = Some(cell);
        }
    }
    best
}

fn minimax(board: &Board, player: Mark, depth: i16) -> i16 {
    match board.outcome() {
        Outcome::Won(mark, _) => {
            return if mark == player {
                10 - depth
            } else {
                depth - 10
            };
        }
        Outcome::Draw => return 0,
        Outcome::Playing => {}
    }
    let scores = board.legal_moves().map(|cell| {
        let mut next = board.clone();
        next.play(cell).unwrap();
        minimax(&next, player, depth + 1)
    });
    if board.turn() == player {
        scores.max().unwrap()
    } else {
        scores.min().unwrap()
    }
}

#[cfg(test)]
mod opponent_tests {
    use super::*;

    fn count_games(board: Board, computer: Mark) -> (usize, usize) {
        match board.outcome() {
            Outcome::Won(mark, _) => {
                assert_eq!(mark, computer, "computer lost: {:?}", board.cells());
                return (1, 0);
            }
            Outcome::Draw => return (0, 1),
            Outcome::Playing => {}
        }
        let moves: Vec<_> = if board.turn() == computer {
            vec![best_move(&board).expect("live board has a move")]
        } else {
            board.legal_moves().collect()
        };
        let mut result = (0, 0);
        for cell in moves {
            let mut next = board.clone();
            next.play(cell).expect("opponent only returns legal moves");
            let (wins, draws) = count_games(next, computer);
            result.0 += wins;
            result.1 += draws;
        }
        result
    }

    #[test]
    fn computer_never_loses_against_any_legal_human_reply() {
        for computer in [Mark::X, Mark::O] {
            let (wins, draws) = count_games(Board::default(), computer);
            println!(
                "Computer {}: {wins} wins, {draws} draws, 0 losses.",
                computer.label()
            );
            assert!(wins > 0 && draws > 0);
        }
    }

    #[test]
    fn takes_a_win_before_blocking_and_stops_after_game_over() {
        let mut board = Board::default();
        for cell in [0, 3, 1, 4] {
            board.play(cell).unwrap();
        }
        assert_eq!(best_move(&board), Some(2));
        board.play(2).unwrap();
        assert_eq!(best_move(&board), None);
    }

    #[test]
    fn answers_opposite_corners_with_an_edge() {
        let mut board = Board::default();
        for cell in [0, 4, 8] {
            board.play(cell).unwrap();
        }
        assert!([1, 3, 5, 7].contains(&best_move(&board).unwrap()));
    }
}

#[derive(Clone, Debug)]
pub struct Match {
    pub board: Board,
    pub human: Mark,
    /// Human wins, computer wins, draws.
    pub score: [u32; 3],
}

impl Default for Match {
    fn default() -> Self {
        Self {
            board: Board::default(),
            human: Mark::X,
            score: [0; 3],
        }
    }
}

impl Match {
    pub fn human_move(&mut self, cell: usize) -> Result<(), MoveError> {
        if self.board.turn() != self.human {
            return Err(MoveError::NotYourTurn);
        }
        self.board.play(cell)?;
        self.record_ending();
        Ok(())
    }

    pub fn computer_move(&mut self) -> bool {
        if self.board.turn() == self.human {
            return false;
        }
        let Some(cell) = best_move(&self.board) else {
            return false;
        };
        self.board.play(cell).expect("minimax returns a legal move");
        self.record_ending();
        true
    }

    fn record_ending(&mut self) {
        // Called only after an accepted move. A finished board rejects every later move.
        match self.board.outcome() {
            Outcome::Won(mark, _) => self.score[usize::from(mark != self.human)] += 1,
            Outcome::Draw => self.score[2] += 1,
            Outcome::Playing => {}
        }
    }

    pub fn next_round(&mut self) -> bool {
        if self.board.outcome() == Outcome::Playing {
            return false;
        }
        self.board = Board::default();
        self.human = self.human.other();
        true
    }

    pub fn status(&self) -> String {
        match self.board.outcome() {
            Outcome::Playing if self.board.turn() == self.human => "Your turn".into(),
            Outcome::Playing => "Computer's turn".into(),
            Outcome::Won(mark, _) if mark == self.human => "You win".into(),
            Outcome::Won(_, _) => "Computer wins".into(),
            Outcome::Draw => "A draw".into(),
        }
    }
}

#[cfg(test)]
mod match_tests {
    use super::*;
    fn finish(game: &mut Match) {
        while game.board.outcome() == Outcome::Playing {
            if game.board.turn() == game.human {
                game.human_move(best_move(&game.board).unwrap()).unwrap();
            } else {
                assert!(game.computer_move());
            }
        }
    }

    #[test]
    fn completed_round_scores_once_and_rematch_alternates_the_starter() {
        let mut game = Match::default();
        finish(&mut game);
        assert_eq!(game.score, [0, 0, 1]);
        let before = game.board.clone();
        assert!(game.human_move(0).is_err());
        assert!(!game.computer_move());
        assert_eq!(game.board, before);
        assert_eq!(game.score, [0, 0, 1]);
        assert!(game.next_round());
        assert_eq!(game.human, Mark::O);
        assert_eq!(game.board, Board::default());
        assert_eq!(game.score, [0, 0, 1]);
        assert_eq!(game.human_move(0), Err(MoveError::NotYourTurn));
        assert!(game.computer_move());
        assert_eq!(game.board.turn(), Mark::O);
        finish(&mut game);
        assert_eq!(game.score, [0, 0, 2]);
        assert!(game.next_round());
        assert_eq!(game.human, Mark::X);
    }

    #[test]
    fn cannot_restart_an_unfinished_round_or_play_twice() {
        let mut game = Match::default();
        assert!(!game.next_round());
        game.human_move(0).unwrap();
        assert_eq!(game.human_move(1), Err(MoveError::NotYourTurn));
        assert_eq!(game.score, [0; 3]);
        assert_eq!(game.board.cells().iter().flatten().count(), 1);
    }
}
