# stefanbot
A Discord bot I created because Rythm doesn't have the functionality to save/load lists of tracks.

## Installation
1. `git clone https://github.com/teunissenstefan/stefanbot`
2. `cd stefanbot`
3. `cp .env.example .env`
4. Put the token of your Discord bot into the .env file.
5. `cargo run`

## Commands
* `join` Make the bot join your voice channel.
* `leave` Make the bot leave your voice channel.
* `play URL` Add a track to the queue.
* `queue` Show all tracks in the queue.
* `save NAME` Save the current queue with a name [a-z].
* `load NAME` Load the queue with the provided name [a-z].
* `pause` Pause playback.
* `resume` Resume playback.
* `skip` Skip the current track.
* `clear` Clear the current queue.
* `help` Display all possible commands.
