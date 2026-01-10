# 📦 Discord Bridge: Modules & Events Registry

This document serves as a reference for all currently implemented events and the modular logic units that consume them.

---

## 🎭 Available Modules

Each module can be independently enabled or disabled via the `enabled_modules` array in your `config.json`.

### 1. `SimpleNotifier`
* **Purpose**: Handles "self-reporting" events.
* **Behavior**: Listens for any event that implements a custom `to_notification()` override. It is the only module that is always active by default.
* **Dependencies**: None.

### 2. `Dashboard`
* **Purpose**: Maintains a single, live-updating "Pinned" message in Discord.
* **Logic**: 
    * Tracks `player_count` via `JoinEvent` and `LeaveEvent`.
    * Updates the `current_map` via `MapChangeEvent`.
    * Throttles updates to every 30 seconds or on-demand to respect Discord rate limits.
* **Commands**: `!dash` (Forces the dashboard to repost and start fresh).

### 3. `StatsTracker`
* **Purpose**: Tracks lifetime and session-based kill counts.
* **Logic**: 
    * Increments counters on `KillEvent`.
    * Saves a `leaderboard.json` file to disk on `MatchEndEvent`.
    * Clears session stats when a new round starts.
* **Commands**: `!top` (Displays Top 5), `!mystats` (Displays user's total kills).

### 4. `KillstreakModule`
* **Purpose**: Announces performance milestones.
* **Logic**: Monitors `KillEvent`. When a player hits 5, 10, 15, or 20 kills without dying, it broadcasts a stylized embed.
* **State**: Resets a player's streak upon their death or a map change.

### 5. `JoinBatcher`
* **Purpose**: Anti-spam for player connections.
* **Logic**: Collects `JoinEvent` names into a buffer. Flushes the buffer into a single summarized message (e.g., *"PlayerA and 4 others joined"*) every second or when the buffer hits 10 players.

### 6. `DuelManager`
* **Purpose**: Detailed tracking for 1v1 encounters.
* **Logic**: 
    * Activates on `DuelStartEvent`.
    * Tracks `DamageEvent` and `AttackEvent` specifically between the two participants.
    * Ends on `KillEvent` or if a duelist hits a non-participant ("Interference").
* **Stats Tracked**: Damage dealt, parries, and attack type counts (Stab, Overhead, etc).

### 7. `AdminHerald`
* **Purpose**: Secure Admin communication.
* **Logic**: 
    * Pings the Admin Role when an `AdminAlert` is fired in-game.
    * Allows Admins to use the `!say <msg>` command in Discord to broadcast to the game server.

---

## 📨 Event Definitions

Events are the data packets sent from the game thread. Modules "Downcast" these to access their fields.

| Event Struct | Fields | Purpose |
| :--- | :--- | :--- |
| **`JoinEvent`** | `name` | Player connection notification. |
| **`LeaveEvent`** | `name` | Player disconnection. |
| **`KillEvent`** | `killer`, `victim`, `weapon` | Combat tracking. |
| **`DamageEvent`** | `attacker`, `victim`, `damage` | Detailed combat/duel tracking. |
| **`AttackEvent`** | `attacker`, `attack_type`, `was_parried` | Combat style analytics. |
| **`MapChangeEvent`** | `new_map` | Triggers Dashboard and State resets. |
| **`MatchEndEvent`** | `winner_team`, `final_score` | Triggers result summaries and disk saves. |
| **`AdminAlert`** | `reporter`, `reason` | Pings Discord Admins. |
| **`DuelStartEvent`** | `challenger`, `opponent` | Initializes the Duel Module. |
| **`CommandRequest`** | `command`, `user`, `user_roles` | Routes Discord messages to modules. |

---

## 🛠 Adding a New Component

1.  **New Event**: Create a struct in `notifications.rs` and call `impl_event!(MyEvent)`.
2.  **New Module**: Create a file in `modules/`, implement `DiscordSubscriber`, and add it to the `all_subscribers` vector in `mod.rs`.