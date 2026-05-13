# UC-01: Start a new game

| ID    | Name                          | Actor          | Purpose                                                        |
| ----- | ---------------------------- | --------------- | ----------- ----------------------------------------------- |
| UC-01 | Start a new game | Player, System | Initialise a game session and launch the first ball |

### UC-01: Start a new game (detailed)

**Actors:** Player (via selection from the menu and plunger), System

**Preconditions:** The terminal is on the home screen or the main menu; the game server is reachable

**Nominal scenario:**

1. The player presses the ‘Start’ button to launch a new session
2. The system resets the game state: score to 0, default number of lives (3), gauges reset to initial state
3. The system prepares the playing area and places the ball in the launcher
4. The player pulls the plunger to launch the first ball (IoT => backend => frontend)
5. The system switches to ‘game in progress’ state and begins tracking game events

**Extensions:**

- 1a. A game is already active → the system asks for confirmation before resetting
- 4a. The player does not pull the plunger within the expected time → a visual/audible reminder is displayed, the game remains on hold

**Postconditions:** A new game is active, the first ball has been launched, the counters have been initialised
