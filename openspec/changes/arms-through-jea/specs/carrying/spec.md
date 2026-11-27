## ADDED Requirements

### Requirement: The arms of a carrying player go forward through JEA

While a player carries a block or a mob, the server SHALL play the clip `craftorio_carry:hold` on that player through
Just Enough Animations, on the channel `craftorio_carry:main`, with a priority in JEA's band of side states, in a
loop, in Replace mode. When the player carries nothing, the server SHALL stop that channel. The client SHALL NOT
draw the arms of a carrying player in third person. Without JEA, the arms of a carrying player SHALL hang.

#### Scenario: Another player sees the arms with armour

- **WHEN** a player in a full set of armour picks up a chest
- **THEN** other players with JEA see both arms forward, and the armour sleeves on the arms

#### Scenario: A crouch while carrying

- **WHEN** a carrying player crouches
- **THEN** the arms stay forward and go down with the body, and the load goes down with them

#### Scenario: The load goes down

- **WHEN** a carrying player puts the load down, or dies
- **THEN** the arms go back to their usual pose

#### Scenario: A login after the load is gone

- **WHEN** a player logs in, and JEA still has the clip of Carry saved on the player, but the player carries nothing
- **THEN** the server stops the clip

#### Scenario: A login with a load

- **WHEN** a player who logged out with a load logs in again
- **THEN** the arms go forward again

### Requirement: The first person view stays

In first person, the client SHALL draw the load in front of the view with the player's arms along its sides, as
before. JEA SHALL NOT be needed for it.

#### Scenario: The carrier looks at the load

- **WHEN** a carrying player looks in first person
- **THEN** the player sees the load and the arms in their own skin
