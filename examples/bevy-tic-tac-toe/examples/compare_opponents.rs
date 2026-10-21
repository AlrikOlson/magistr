//! Evaluate the proposed win/block/center/corner policy against every human reply.
use magistr_tic_tac_toe::{Board, Mark, Outcome};

fn tactical(board: &Board) -> Option<usize> {
    let me = board.turn();
    let moves: Vec<_> = board.legal_moves().collect();
    for &cell in &moves {
        let mut next = board.clone();
        next.play(cell).unwrap();
        if matches!(next.outcome(), Outcome::Won(mark, _) if mark == me) {
            return Some(cell);
        }
    }
    for line in magistr_tic_tac_toe::LINES {
        let threats = line
            .iter()
            .filter(|&&i| board.cells()[i] == Some(me.other()))
            .count();
        if threats == 2 {
            if let Some(&cell) = line.iter().find(|&&i| board.cells()[i].is_none()) {
                return Some(cell);
            }
        }
    }
    [4, 0, 2, 6, 8, 1, 3, 5, 7]
        .into_iter()
        .find(|i| moves.contains(i))
}

fn find_loss(board: Board, computer: Mark, history: &mut Vec<usize>) -> bool {
    match board.outcome() {
        Outcome::Won(mark, _) => return mark != computer,
        Outcome::Draw => return false,
        Outcome::Playing => {}
    }
    let moves: Vec<_> = if board.turn() == computer {
        tactical(&board).into_iter().collect()
    } else {
        board.legal_moves().collect()
    };
    for cell in moves {
        let mut next = board.clone();
        next.play(cell).unwrap();
        history.push(cell);
        if find_loss(next, computer, history) {
            return true;
        }
        history.pop();
    }
    false
}

fn main() {
    for computer in [Mark::X, Mark::O] {
        let mut history = Vec::new();
        let loses = find_loss(Board::default(), computer, &mut history);
        if !loses {
            println!(
                "Tactical policy as {}: no losing game found.",
                computer.label()
            );
            continue;
        }
        println!(
            "Tactical policy as {} loses. Move sequence (cells 1–9): {:?}",
            computer.label(),
            history.iter().map(|i| i + 1).collect::<Vec<_>>()
        );
    }
}
