# UC-02: Select a character

| ID    | Name                   | Actor | Purpose                                         |
| ----- | -------------------- - | ------ | ------------------------------------------- |
| UC-02 | Choose a character | Player | Select a character before a game |

### UC-02: Select a character (detailed)

**Actors:** Player (via the terminal’s navigation buttons)

**Prerequisites:** The player is on the selection screen before starting a game; no game is active

**Nominal scenario:**

1. The system displays the character selection screen on the backglass showing the available characters and their abilities
2. The player uses the navigation buttons to scroll through the characters
3. For each character highlighted, the system displays their statistics and special ability (ultimate) on the backglass
4. The player confirms their choice using the confirmation button
5. The system records the selected character and redirects to the corresponding game mode

**Extensions:**

- 2a. Only one character is available → the selection is ignored, the character is assigned automatically

**Postconditions:** The character is selected, and their statistics and ultimate ability are loaded for the start of the game (UC-01)
