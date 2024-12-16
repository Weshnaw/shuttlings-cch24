#![allow(dead_code)]

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum Placement {
    Milk,
    Cookie,
    #[default]
    Empty,
}

impl Placement {
    fn piece_str(&self) -> &'static str {
        match *self {
            Placement::Milk => "🥛",
            Placement::Cookie => "🍪",
            Placement::Empty => "⬛",
        }
    }
}

#[derive(Clone, Debug)]
struct BoardState {
    board: Arc<Mutex<[[Placement; 4]; 4]>>,
    highest: Arc<Mutex<[u8; 4]>>,
    winner: Arc<Mutex<Option<Placement>>>,
    random: Arc<Mutex<StdRng>>,
}

impl Default for BoardState {
    fn default() -> Self {
        BoardState {
            board: Arc::default(),
            highest: Arc::default(),
            winner: Arc::default(),
            random: Arc::new(Mutex::new(StdRng::seed_from_u64(2024))),
        }
    }
}

impl BoardState {
    async fn display_state(&self) -> (String, Option<Placement>) {
        let lock = self.board.lock().await;
        let mut board = String::new();
        let mut continuous_row = [(0_u8, Placement::Empty); 4];
        let mut continuous_col = [(0_u8, Placement::Empty); 4];
        let mut continuous_diag = [(0_u8, Placement::Empty); 2];
        let mut empty_placement = false;

        // Setup initial placements; if these aren't the same as the rest then we automatically know it's not a connect 4
        for x in 0..4 {
            continuous_col[x] = (0, lock[0][x]);
            continuous_row[x] = (0, lock[x][0]);
        }
        continuous_diag[0] = (0, lock[0][0]);
        continuous_diag[1] = (0, lock[0][3]);

        for row in 0..5 {
            let mut row_str = String::new();
            for col in 0..6 {
                let tile = match (row, col) {
                    (0, _) | (_, 0) | (_, 5) => "⬜",
                    _ => {
                        let row = row - 1;
                        let col = col - 1;
                        let placement = lock[row][col];
                        if placement == Placement::Empty {
                            empty_placement = true;
                        } else {
                            if continuous_col[col].1 == placement {
                                continuous_col[col].0 += 1;
                            }
                            if continuous_row[row].1 == placement {
                                continuous_row[row].0 += 1;
                            }
                            if row == col && continuous_diag[0].1 == placement {
                                continuous_diag[0].0 += 1;
                            }
                            debug!(?col, ?row);
                            if (3 - col) == row && continuous_diag[1].1 == placement {
                                debug!("hit");
                                continuous_diag[1].0 += 1;
                            }
                        }

                        placement.piece_str()
                    }
                };
                row_str += tile;
            }
            board = format!("{row_str}\n{board}");
        }
        drop(lock);

        let matching = continuous_row.iter().find(|(count, _)| count == &4);
        let matching = continuous_col
            .iter()
            .find(|(count, _)| count == &4)
            .or(matching);
        let matching = continuous_diag
            .iter()
            .find(|(count, _)| count == &4)
            .or(matching);

        info!(?continuous_row);
        info!(?continuous_col);
        info!(?continuous_diag);
        info!(?matching);
        if let Some((_, placement)) = matching {
            (
                format!("{board}{} wins!\n", placement.piece_str()),
                Some(*placement),
            )
        } else if !empty_placement {
            (format!("{board}No winner.\n"), Some(Placement::Empty))
        } else {
            (board, None)
        }
    }

    async fn create_random_board(&self) {
        let mut lock = self.board.lock().await;
        let mut rng = self.random.lock().await;
        for row in (0..4).rev() {
            for col in 0..4 {
                let rng = match rng.gen::<bool>() {
                    true => Placement::Cookie,
                    false => Placement::Milk,
                };

                lock[row][col] = rng;
            }
        }

        info!(?lock);
    }
}

async fn board(State(state): State<BoardState>) -> Response {
    debug!("Calling board");

    state.display_state().await.0.into_response()
}

#[instrument]
async fn reset(State(state): State<BoardState>) -> Response {
    debug!("Calling reset");

    let mut lock = state.board.lock().await;
    *lock = [[Placement::Empty; 4]; 4];
    drop(lock);
    let mut lock = state.winner.lock().await;
    *lock = None;
    drop(lock);
    let mut lock = state.highest.lock().await;
    *lock = [0; 4];
    drop(lock);
    let mut lock = state.random.lock().await;
    *lock = StdRng::seed_from_u64(2024);
    drop(lock);

    state.display_state().await.0.into_response()
}

async fn place(
    Path((team, column)): Path<(String, u8)>,
    State(state): State<BoardState>,
) -> Response {
    let lock = state.winner.lock().await;
    if lock.is_some() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            state.display_state().await.0,
        )
            .into_response();
    }
    drop(lock);

    let placement = match column {
        1..=4 => match team.as_str() {
            "milk" => Placement::Milk,
            "cookie" => Placement::Cookie,
            _ => {
                warn!("Invalid team {team}, {column}");
                return StatusCode::BAD_REQUEST.into_response();
            }
        },
        _ => {
            warn!("Invalid column {team}, {column}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    let column = column as usize - 1;
    let mut lock = state.highest.lock().await;
    let highest = lock[column];
    if highest == 4 {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            state.display_state().await.0,
        )
            .into_response();
    }
    lock[column] += 1;
    drop(lock);
    let mut lock = state.board.lock().await;
    lock[highest as usize][column] = placement;
    drop(lock);

    let (board, winner) = state.display_state().await;

    if let Some(winner) = winner {
        let mut lock = state.winner.lock().await;
        *lock = Some(winner);
    }

    board.into_response()
}

async fn random(State(state): State<BoardState>) -> Response {
    debug!("Calling random");
    state.create_random_board().await;
    let mut lock = state.highest.lock().await;
    *lock = [4; 4];
    drop(lock);
    let (board, winner) = state.display_state().await;
    if let Some(winner) = winner {
        let mut lock = state.winner.lock().await;
        *lock = Some(winner);
    }
    board.into_response()
}

#[instrument]
pub fn router() -> Router {
    debug!("Loading routes");
    let state = BoardState::default();

    Router::new()
        .route("/board", get(board))
        .route("/reset", post(reset))
        .route("/place/:team/:column", post(place))
        .route("/random-board", get(random))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum_test::TestServer;

    use super::*;

    const EMPTY_STATE: &str = "\
⬜⬛⬛⬛⬛⬜
⬜⬛⬛⬛⬛⬜
⬜⬛⬛⬛⬛⬜
⬜⬛⬛⬛⬛⬜
⬜⬜⬜⬜⬜⬜
";

    #[rstest::fixture]
    fn server() -> TestServer {
        TestServer::new(router()).unwrap()
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_initial_board(server: TestServer) {
        let result = server.get(&"/board").await;

        debug!(?result);
        result.assert_status_ok();
        result.assert_text(EMPTY_STATE);
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_reset(server: TestServer) {
        let result = server.post(&"/reset").await;

        debug!(?result);
        result.assert_status_ok();
        result.assert_text(EMPTY_STATE);
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_invalid_column(server: TestServer) {
        let result = server.post(&"/place/cookie/fun").await;

        debug!(?result);
        result.assert_status_bad_request();
    }

    #[derive(Debug)]
    enum Advance<'a> {
        Place(u8, &'a str, StatusCode, Option<&'a str>),
        Reset,
    }

    #[rstest::rstest]
    #[case::invalid_team(&[
        Advance::Place(1, "tiger", StatusCode::BAD_REQUEST, None)
    ])]
    #[case::invalid_team(&[
        Advance::Place(20, "cookie", StatusCode::BAD_REQUEST, None)
    ])]
    #[case::basic_cookie_win(&[
         Advance::Reset,
         Advance::Place(1, "cookie", StatusCode::OK, Some("\
⬜⬛⬛⬛⬛⬜
⬜⬛⬛⬛⬛⬜
⬜⬛⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜⬜⬜⬜⬜⬜
")),
         Advance::Place(1, "cookie", StatusCode::OK, Some("\
⬜⬛⬛⬛⬛⬜
⬜⬛⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜⬜⬜⬜⬜⬜
")),
         Advance::Place(1, "cookie", StatusCode::OK, Some("\
⬜⬛⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜⬜⬜⬜⬜⬜
")),
         Advance::Place(1, "cookie", StatusCode::OK, Some("\
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜⬜⬜⬜⬜⬜
🍪 wins!
")),
         Advance::Place(1, "cookie", StatusCode::SERVICE_UNAVAILABLE, Some("\
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜⬜⬜⬜⬜⬜
🍪 wins!
")),
         Advance::Place(2, "cookie", StatusCode::SERVICE_UNAVAILABLE, Some("\
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜🍪⬛⬛⬛⬜
⬜⬜⬜⬜⬜⬜
🍪 wins!
")),
      ])]
    #[case::easy_diag_cookie_win(&[
         Advance::Reset,
         Advance::Place(1, "cookie", StatusCode::OK, None),
         Advance::Place(2, "cookie", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(4, "milk", StatusCode::OK, None),
         Advance::Place(2, "cookie", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(4, "cookie", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(4, "cookie", StatusCode::OK, None),
         Advance::Place(4, "cookie", StatusCode::OK, Some("\
⬜⬛⬛⬛🍪⬜
⬜⬛⬛🍪🍪⬜
⬜⬛🍪🍪🍪⬜
⬜🍪🍪🍪🥛⬜
⬜⬜⬜⬜⬜⬜
🍪 wins!
")),
      ])]
    #[case::hard_diag_cookie_win(&[
         Advance::Reset,
         Advance::Place(4, "cookie", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(2, "cookie", StatusCode::OK, None),
         Advance::Place(1, "milk", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(2, "cookie", StatusCode::OK, None),
         Advance::Place(1, "cookie", StatusCode::OK, None),
         Advance::Place(2, "cookie", StatusCode::OK, None),
         Advance::Place(1, "cookie", StatusCode::OK, None),
         Advance::Place(1, "cookie", StatusCode::OK, Some("\
⬜🍪⬛⬛⬛⬜
⬜🍪🍪⬛⬛⬜
⬜🍪🍪🍪⬛⬜
⬜🥛🍪🍪🍪⬜
⬜⬜⬜⬜⬜⬜
🍪 wins!
")),
      ])]
    #[case::no_winner(&[
         Advance::Reset,
         Advance::Place(1, "cookie", StatusCode::OK, None),
         Advance::Place(1, "cookie", StatusCode::OK, None),
         Advance::Place(1, "cookie", StatusCode::OK, None),
         Advance::Place(1, "milk", StatusCode::OK, None),
         Advance::Place(2, "milk", StatusCode::OK, None),
         Advance::Place(2, "milk", StatusCode::OK, None),
         Advance::Place(2, "milk", StatusCode::OK, None),
         Advance::Place(2, "cookie", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(3, "cookie", StatusCode::OK, None),
         Advance::Place(3, "milk", StatusCode::OK, None),
         Advance::Place(4, "milk", StatusCode::OK, None),
         Advance::Place(4, "milk", StatusCode::OK, None),
         Advance::Place(4, "milk", StatusCode::OK, None),
         Advance::Place(4, "cookie", StatusCode::OK, Some("\
⬜🥛🍪🥛🍪⬜
⬜🍪🥛🍪🥛⬜
⬜🍪🥛🍪🥛⬜
⬜🍪🥛🍪🥛⬜
⬜⬜⬜⬜⬜⬜
No winner.
")),
      ])]
    #[test_log::test(tokio::test)]
    async fn test_gameplay(server: TestServer, #[case] steps: &[Advance<'_>]) {
        debug!(?steps);

        for step in steps {
            let (query, status, game_state) = match step {
                Advance::Place(pos, piece, status, state) => {
                    (format!("/place/{piece}/{pos}"), *status, state)
                }
                Advance::Reset => ("/reset".into(), StatusCode::OK, &Some(EMPTY_STATE)),
            };
            let result = server.post(&query).await;
            debug!(?result);

            result.assert_status(status);
            if let Some(game_state) = game_state {
                result.assert_text(game_state);
            }
        }
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_random(server: TestServer) {
        let result = server.get(&"/random-board").await;

        debug!(?result);
        result.assert_status_ok();
        result.assert_text_contains(
            "\
⬜🍪🍪🍪🍪⬜
⬜🥛🍪🍪🥛⬜
⬜🥛🥛🥛🥛⬜
⬜🍪🥛🍪🥛⬜
⬜⬜⬜⬜⬜⬜
",
        );

        let result = server.get(&"/random-board").await;

        debug!(?result);
        result.assert_status_ok();
        result.assert_text(
            "\
⬜🍪🥛🍪🍪⬜
⬜🥛🍪🥛🍪⬜
⬜🥛🍪🍪🍪⬜
⬜🍪🥛🥛🥛⬜
⬜⬜⬜⬜⬜⬜
No winner.
",
        );
    }
}
