# DesktopMate Installer

The **DesktopMate Installer** is an automated installer written in Rust. It streamlines the process of setting up the DesktopMate and configuring it for modding. 

The installer:
- Prompts the user to point to where they would like the game to be installed.
- Downloads and installs the latest supported version of DesktopMate using **DepotDownloader**. 
- Applies **Mr. Goldberg's Steam Emulator Patch** allowing the game to run with or without Steam present.
- Installs **MelonLoader v0.6.6**, the currently recommended version by downloading and extracting its files directly into the game directory.
- Installs/updates the **Custom Avatar Loader mod** by extracting both the `Mods` and `UserLibs` directories into the game directory.
- Creates desktop shortcuts for launching the game (with or without console output).

## License
This project is licensed under the MIT License. See the LICENSE file for details.

## Contributing
Contributions, issues, and feature requests are welcome! Feel free to check the issues page.
