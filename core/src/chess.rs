use std::ops::{Index, IndexMut};

use crate::{
    errors::AppError,
    pb::{
        game::{Board, Cell, Color, GameState, Location, Piece, Row},
        query::Position,
    },
};

impl GameState {
    pub fn new(white: String, black: String) -> Self {
        Self {
            white_player: white,
            black_player: black,
            turn: Color::White as i32,
            board: Some(Board::new()),
        }
    }

    pub fn with_board(self, board: Board) -> Self {
        Self {
            board: Some(board),
            ..self
        }
    }

    pub fn with_white_player(self, white_player: String) -> Self {
        Self {
            white_player,
            ..self
        }
    }

    pub fn with_black_player(self, black_player: String) -> Self {
        Self {
            black_player,
            ..self
        }
    }

    pub fn apply_move(&mut self, from: Position, to: Position) -> Result<(), AppError> {
        if let Err(e) = self.validate_move(&from, &to) {
            return Err(e);
        }

        let from = Location::from_pos(
            from.clone(),
            self.board.clone().unwrap().rows[from.clone().x as usize].cells
                [from.clone().y as usize]
                .piece
                .clone(),
        );
        let to = Location::from_pos(
            to.clone(),
            self.board.clone().unwrap().rows[to.clone().x as usize].cells[to.clone().y as usize]
                .piece
                .clone(),
        );

        if let Some(p) = to.clone().piece {
            if p.color == self.turn {
                return Err(AppError::InternalGameError(
                    "You cannot move onto your own piece".to_string(),
                ));
            }
        }

        self.board.as_mut().unwrap().rows[to.coords[0] as usize].cells[to.coords[1] as usize]
            .piece = from.piece;
        self.board.as_mut().unwrap().rows[from.coords[0] as usize].cells[from.coords[1] as usize]
            .piece = None;

        self.turn = (self.turn + 1) % 2;

        Ok(())
    }

    pub fn validate_move(&self, from: &Position, to: &Position) -> Result<(), AppError> {
        let from = Location::from_pos(
            from.clone(),
            self.board.clone().unwrap().rows[from.clone().x as usize].cells
                [from.clone().y as usize]
                .piece
                .clone(),
        );
        let to = Location::from_pos(
            to.clone(),
            self.board.clone().unwrap().rows[to.clone().x as usize].cells[to.clone().y as usize]
                .piece
                .clone(),
        );

        self.validate_move_inner(&from, &to)
    }

    fn validate_move_inner(&self, from: &Location, to: &Location) -> Result<(), AppError> {
        let piece = match &from.piece {
            Some(p) => p,
            None => {
                return Err(AppError::InternalGameError(
                    "No piece at the source location".to_string(),
                ));
            }
        };

        let current_color = Color::from_i32(self.turn).expect("Correct color");

        if piece.color != current_color as i32 {
            return Err(AppError::InternalGameError(
                "It's not this piece's turn to move".to_string(),
            ));
        }

        if !piece.can_move_to(from, to, self.board.as_ref().unwrap()) {
            return Err(AppError::InternalGameError(
                "Invalid move for the piece".to_string(),
            ));
        }

        Ok(())
    }
}

impl Piece {
    pub fn new(color: Color, kind: String) -> Self {
        Self {
            color: color as i32,
            kind,
        }
    }

    pub fn new_from_i32(color: i32, kind: String) -> Self {
        Self { color, kind }
    }

    pub fn can_move_to(&self, from: &Location, to: &Location, board: &Board) -> bool {
        let dx = to.coords[0] as i32 - from.coords[0] as i32;
        let dy = to.coords[1] as i32 - from.coords[1] as i32;

        match self.kind.as_str() {
            "P" => self.validate_pawn_move(from, to, dx, dy, board),
            "R" => self.validate_rook_move(from, to, dx, dy, board),
            "N" => self.validate_knight_move(from, to, dx, dy, board),
            "B" => self.validate_bishop_move(from, to, dx, dy, board),
            "Q" => self.validate_queen_move(from, to, dx, dy, board),
            "K" => self.validate_king_move(from, to, dx, dy, board),
            _ => false,
        }
    }

    fn validate_pawn_move(
        &self,
        from: &Location,
        to: &Location,
        dx: i32,
        dy: i32,
        board: &Board,
    ) -> bool {
        // Pawn moves: one step forward or diagonal for capture
        let direction = if self.color == Color::White as i32 {
            1
        } else {
            -1
        };
        let initial_row = if self.color == Color::White as i32 {
            1
        } else {
            6
        };

        // Forward move
        if dy == 0 && dx == direction {
            return board.is_empty(to);
        }

        // Initial double move
        if dy == 0 && dx == 2 * direction && from.coords[0] as i32 == initial_row {
            let mid_location = Location::new(
                vec![(from.coords[0] as i32 + direction) as u32, from.coords[1]],
                Piece::new_from_i32(self.color, "P".to_string()),
            );
            return board.is_empty(to) && board.is_empty(&mid_location);
        }

        // Capture move
        if dy.abs() == 1 && dx == direction {
            return board.has_enemy_piece(to, self.color);
        }

        false
    }

    fn validate_rook_move(
        &self,
        from: &Location,
        to: &Location,
        dx: i32,
        dy: i32,
        board: &Board,
    ) -> bool {
        // Rook moves: horizontally or vertically
        if dx != 0 && dy != 0 {
            return false;
        }

        let (x_direction, y_direction) = (dx.signum(), dy.signum());

        // Check if the path is clear
        let mut x_coord = from.coords[0] as i32 + x_direction;
        let mut y_coord = from.coords[1] as i32 + y_direction;

        while x_coord != to.coords[0] as i32 || y_coord != to.coords[1] as i32 {
            if !board.is_empty(&Location::new(
                vec![x_coord as u32, y_coord as u32],
                Piece::new_from_i32(self.color, self.kind.clone()),
            )) {
                return false;
            }
            x_coord += x_direction;
            y_coord += y_direction;
        }

        board.is_empty_or_enemy(to, self.color)
    }

    fn validate_knight_move(
        &self,
        _: &Location,
        to: &Location,
        dx: i32,
        dy: i32,
        board: &Board,
    ) -> bool {
        // Knight moves: in "L" shapes
        ((dx.abs() == 2 && dy.abs() == 1) || (dx.abs() == 1 && dy.abs() == 2))
            && board.is_empty_or_enemy(to, self.color)
    }

    fn validate_bishop_move(
        &self,
        from: &Location,
        to: &Location,
        dx: i32,
        dy: i32,
        board: &Board,
    ) -> bool {
        // Bishop moves: diagonally
        if dx.abs() != dy.abs() {
            return false;
        }

        // Check if the path is clear
        let x_direction = dx.signum();
        let y_direction = dy.signum();

        let mut x = from.coords[0] as i32 + x_direction;
        let mut y = from.coords[1] as i32 + y_direction;

        while x != to.coords[0] as i32 || y != to.coords[1] as i32 {
            if !board.is_empty(&Location::new(
                vec![x as u32, y as u32],
                Piece::new_from_i32(self.color, self.kind.clone()),
            )) {
                return false;
            }

            x += x_direction;
            y += y_direction;
        }

        board.is_empty_or_enemy(to, self.color)
    }

    fn validate_queen_move(
        &self,
        from: &Location,
        to: &Location,
        dx: i32,
        dy: i32,
        board: &Board,
    ) -> bool {
        // Queen moves: combination of rook and bishop moves
        self.validate_rook_move(from, to, dx, dy, board)
            || self.validate_bishop_move(from, to, dx, dy, board)
    }

    fn validate_king_move(
        &self,
        _: &Location,
        to: &Location,
        dx: i32,
        dy: i32,
        board: &Board,
    ) -> bool {
        // King moves: one square in any direction
        dx.abs() <= 1 && dy.abs() <= 1 && board.is_empty_or_enemy(to, self.color)
    }
}

impl Location {
    pub fn new(coords: Vec<u32>, piece: Piece) -> Self {
        Self {
            coords,
            piece: Some(piece),
        }
    }

    pub fn with_coords(self, coords: Vec<u32>) -> Self {
        Self { coords, ..self }
    }

    pub fn with_piece(self, piece: Piece) -> Self {
        Self {
            piece: Some(piece),
            ..self
        }
    }
}

impl Board {
    pub fn new() -> Self {
        let mut board: Vec<Row> = Vec::default();
        board.resize(8, Row::default());

        for i in 1..7 {
            let mut row: Vec<Cell> = Vec::default();
            row.resize(8, Cell::default());
            board[i] = Row::from(row);
        }

        let white_figures: Vec<(&str, (usize, usize))> = vec![
            ("R", (0, 0)),
            ("N", (0, 1)),
            ("B", (0, 2)),
            ("Q", (0, 3)),
            ("K", (0, 4)),
            ("B", (0, 5)),
            ("N", (0, 6)),
            ("R", (0, 7)),
        ];

        board[0].cells.resize(8, Cell::default());

        for (kind, coords) in white_figures {
            board[0][coords.1] = Cell::new(Piece::new(Color::White, kind.to_string()));
        }

        let black_figures: Vec<(&str, (usize, usize))> = vec![
            ("R", (7, 0)),
            ("N", (7, 1)),
            ("B", (7, 2)),
            ("Q", (7, 3)),
            ("K", (7, 4)),
            ("B", (7, 5)),
            ("N", (7, 6)),
            ("R", (7, 7)),
        ];

        board[7].cells.resize(8, Cell::default());

        for (kind, coords) in black_figures {
            board[7][coords.1] = Cell::new(Piece::new(Color::Black, kind.to_string()));
        }

        for j in 0..8 {
            board[1][j] = Cell::new(Piece::new(Color::White, "P".to_string()));
            board[6][j] = Cell::new(Piece::new(Color::Black, "P".to_string()));
        }

        Self { rows: board }
    }

    pub fn get_piece_at(&self, location: &Location) -> Option<&Piece> {
        self.rows[location.coords[0] as usize].cells[location.coords[1] as usize]
            .piece
            .as_ref()
    }

    pub fn has_enemy_piece(&self, location: &Location, color: i32) -> bool {
        if let Some(piece) = self.get_piece_at(location) {
            return piece.color != color;
        }
        false
    }

    pub fn is_empty(&self, location: &Location) -> bool {
        self.get_piece_at(location).is_none()
    }

    pub fn is_empty_or_enemy(&self, location: &Location, color: i32) -> bool {
        self.is_empty(location) || self.has_enemy_piece(location, color)
    }
}

impl From<Vec<Row>> for Board {
    fn from(rows: Vec<Row>) -> Self {
        Self { rows }
    }
}

impl From<Vec<Cell>> for Row {
    fn from(cells: Vec<Cell>) -> Self {
        Self { cells }
    }
}

impl Index<usize> for Row {
    type Output = Cell;

    fn index(&self, index: usize) -> &Self::Output {
        &self.cells[index]
    }
}

impl IndexMut<usize> for Row {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.cells[index]
    }
}

impl Cell {
    pub fn new(piece: Piece) -> Self {
        Self { piece: Some(piece) }
    }
}

impl Location {
    pub fn from_pos(pos: Position, piece: Option<Piece>) -> Self {
        Self {
            coords: vec![pos.x, pos.y],
            piece,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pb::game::{Color, GameState};

    #[test]
    fn test_initial_game_state() {
        let white_player = "Alice".to_string();
        let black_player = "Bob".to_string();
        let game_state = GameState::new(white_player.clone(), black_player.clone());

        assert_eq!(game_state.turn, Color::White as i32);

        assert_eq!(game_state.white_player, white_player);
        assert_eq!(game_state.black_player, black_player);
    }

    #[test]
    fn test_pawn_valid_move() {
        let mut game_state = GameState::new("Alice".to_string(), "Bob".to_string());

        let from = Position { x: 1, y: 0 };
        let to = Position { x: 3, y: 0 };

        assert!(game_state.validate_move(&from, &to).is_ok());

        game_state.turn = Color::Black as i32;
        let from_black = Position { x: 6, y: 0 };
        let to_invalid = Position { x: 5, y: 0 };

        assert!(game_state.validate_move(&from_black, &to_invalid).is_ok());
    }

    #[test]
    fn test_rook_invalid_move() {
        let game_state = GameState::new("Alice".to_string(), "Bob".to_string());

        let from = Position { x: 0, y: 0 };
        let to = Position { x: 2, y: 2 };
        assert!(game_state.validate_move(&from, &to).is_err());
    }

    #[test]
    fn test_turn_logic() {
        let mut game_state = GameState::new("Alice".to_string(), "Bob".to_string());

        game_state.turn = Color::White as i32;

        let from = Position { x: 1, y: 0 };
        let to = Position { x: 2, y: 0 };
        println!("{:?}", game_state.validate_move(&from, &to));
        assert!(game_state.validate_move(&from, &to).is_ok());

        game_state.turn = Color::Black as i32;
        let from_black = Position { x: 6, y: 0 };
        let to_black = Position { x: 5, y: 0 };
        assert!(game_state.validate_move(&from_black, &to_black).is_ok());
    }
}
