use crate::mp::{domain, Con};
use rand::{seq::IteratorRandom, Rng};
use std::collections::{HashMap, HashSet};

// lobby parameters
#[derive(Default)]
pub struct PrepLobbyState {
    // should contain all players in lobby
    pub start_votes: HashMap<Con, bool>,
}

impl PrepLobbyState {
    pub fn to_running(&self) -> RunningLobbyState {
        self.into()
    }

    pub fn join_con(&mut self, con: Con) {
        self.start_votes.insert(con, false);
    }

    pub fn remove_con(&mut self, con: &Con) {
        self.start_votes.remove(con);
    }

    pub fn vote_start(&mut self, con: Con, vote: bool) {
        // expected to already contain the con
        if self.start_votes.contains_key(&con) {
            self.start_votes.insert(con, vote);
        }
    }

    pub fn all_voted_to_start(&self) -> bool {
        self.start_votes
            .values()
            .cloned()
            .all(std::convert::identity)
    }
}

pub struct RunningLobbyState {
    // TODO merge into "con to con state"
    pub snakes: HashMap<Con, domain::Snake>,
    pub foods: domain::Foods,
    pub boundaries: domain::Boundaries,
    pub counter: u32,
    pub cons: HashSet<Con>,
}

impl From<&PrepLobbyState> for RunningLobbyState {
    fn from(PrepLobbyState { start_votes }: &PrepLobbyState) -> Self {
        #[allow(unused)]
        use domain::{Direction, Food, Foods, Pos, Sections, Snake};

        let cons = start_votes.keys().cloned().collect::<HashSet<_>>();

        let snakes = {
            let mut snakes = vec![];

            for (i, con) in cons.iter().cloned().enumerate() {
                let sections = Sections::from_directions(
                    Pos::new(i as _, 0),
                    (0..3).into_iter().map(|_| Direction::Up),
                );

                let snake = Snake {
                    sections,
                    direction: Direction::Up,
                };

                snakes.push((con, snake));
            }

            snakes.into_iter().collect()
        };

        let foods = Foods::default();

        let boundaries = domain::Pos::new(0, 0).boundaries_in_radius(10, 10);

        Self {
            snakes,
            foods,
            boundaries,
            counter: 0,
            cons,
        }
    }
}

impl RunningLobbyState {
    pub fn advance(&mut self) {
        // TODO do not spawn on current snake positions,
        //
        // TODO figure can still spawn on boundaries, seems like by one problem
        fn refill_foods(foods: &mut domain::Foods, boundaries: &domain::Boundaries) {
            if foods.count() < 30 {
                use strum::IntoEnumIterator;
                let figures = domain::figures::Figures::iter();

                let figure = figures.choose(&mut rand::thread_rng()).unwrap();

                let x = rand::thread_rng()
                    .gen_range((boundaries.min.x)..(boundaries.max.x - (figure.x_dim() as i32)));
                let y = rand::thread_rng()
                    .gen_range((boundaries.min.y)..(boundaries.max.y - (figure.y_dim() as i32)));

                for (i, row) in figure.to_iter().into_iter().enumerate() {
                    for (j, col) in row.into_iter().enumerate() {
                        if col.is_food() {
                            let food = domain::Food::new(x + (j as i32), y + (i as i32));

                            if boundaries.relation(food.pos()).is_inside() {
                                foods.insert(food);
                            }
                        }
                    }
                }
            }
        }

        use domain::AdvanceResult;

        self.counter += 1;

        // indeces to remove
        let mut rm = vec![];

        let other_snakes = self.snakes.clone();
        for (i, snake) in self.snakes.values_mut().enumerate() {
            let other_snakes = other_snakes
                .values()
                .enumerate()
                .filter(|(_i, _)| *_i != i)
                .map(|(_, snake)| snake.clone())
                .collect::<Vec<_>>();

            fn snake_to_food_trace(foods: &mut domain::Foods, snake: &mut domain::Snake) {
                foods.extend(snake.iter_vertices().map(domain::Food::from));
            }

            match snake.advance(&mut self.foods, other_snakes.as_slice(), &self.boundaries) {
                AdvanceResult::Success => {}
                AdvanceResult::BitYaSelf
                | AdvanceResult::BitSomeone
                | AdvanceResult::OutOfBounds => {
                    rm.push(i);
                    snake_to_food_trace(&mut self.foods, snake);
                }
            }
        }

        let mut idx = 0;
        self.snakes.retain(|_, _| {
            let retain = !rm.contains(&idx);
            idx += 1;
            retain
        });

        refill_foods(&mut self.foods, &self.boundaries);
    }

    pub fn set_con_direction(&mut self, con: Con, direction: domain::Direction) {
        if self.snakes.contains_key(&con) {
            self.snakes
                .get_mut(&con)
                .unwrap()
                .set_direction(direction)
                .unwrap_or(());
            tracing::info!("set direction {:?}", direction);
        }
    }

    // no join_con because joining midgame is forbidden

    pub fn remove_con(&mut self, con: &Con) {
        self.cons.remove(con);
        self.snakes.remove(con);
    }
}

pub enum LobbyState {
    Prep(PrepLobbyState),
    Running(RunningLobbyState),
    // terminated is scheduled for clean up
    Terminated,
}
