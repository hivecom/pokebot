# pokebot

```
pokebot 0.3.0

USAGE:
    pokebot [FLAGS] [OPTIONS] [config-path]

FLAGS:
    -h, --help       Prints help information
    -l, --local      Run locally in text mode
    -V, --version    Prints version information
    -v, --verbose    Print the content of all packets

OPTIONS:
    -a, --address <address>                         The address of the server to connect to
    -g, --generate-identities <gen-id-count>        Generate 'count' identities
    -d, --master_channel <master-channel>           The channel the master bot should connect to
    -w, --increase-security-level <wanted-level>    Increases the security level of all identities in the config file

ARGS:
    <config-path>    Configuration file [default: config.toml]
```
## Usage

 1. Poke the main bot.
 2. Once the secondary bot joins your channel, type `!help` for a list of commands.
 
 **Chat commands:**
 ```
    add       Adds url to playlist
    clear     Clears the playback queue
    help      Prints this message or the help of the given subcommand(s)
    leave     Leaves the channel
    next      Switches to the next playlist entry
    pause     Pauses audio playback
    play      Starts audio playback
    search    Adds the first video found on YouTube
    seek      Seeks by a specified amount
    stop      Stops audio playback
    volume    Changes the volume to the specified value
 ```

## Compiling

### Nix
If you are using nix you can enter the development shell with `nix develop`.

### Other systems

1. Make sure the following are installed
    * cargo + rustc 1.42 or later
    * `openssl`
    * `libopus`
    * `gstreamer` development libraries which should be `libgstreamer-dev` and `libgstreamer-plugins-base-dev`

2. Clone the source with `git`:
    ```sh
    $ git clone https://github.com/Mavulp/pokebot.git
    $ cd pokebot
    ```

3. Building the binary
    ```sh
    $ cargo build --release
    ```

    This creates the binary under `target/release/`.

## This repo is a flake

Create `config.toml` based on `config.toml.example` and run with `nix run github:hivecom/pokebot`.
