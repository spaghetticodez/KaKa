# KaKa UI strings (en). Placeholders: { $name }. Converted from the Flutter ARB files.

title = KaKa Trainer Launcher
launch = Launch
libraryEmpty =
    No trainers yet.
    Click + to add a trainer (.exe) for one of your games.
addTrainer = Add Trainer
editTrainer = Edit Trainer
edit = Edit
gameName = Game name
trainerFile = Trainer (.exe)
gameCover = Cover image (optional)
steamAppId = Steam AppID (optional)
steamAppIdHelp = Only needed when several games run at once: picks the right one.
select = Select
change = Change
save = Save
cancel = Cancel
remove = Remove
removeTrainerConfirm =
    Remove "{ $name }" from the library?

    The trainer file itself is not deleted.
trainerAdded = Trainer added
trainerUpdated = Trainer updated
requiredFields = Game name and trainer file are required.
invalidAppId = Steam AppID must be a positive number.
setting = Settings
language = Language
steamPath = Steam path
steamPathAuto = Auto-detect
set = Set
reset = Reset
steamPathSet = Steam path set
ok = OK
launchFail = Launch failed
launched = Trainer started: { $name }
trainerPathNotFound =
    Trainer file not found.

    Path: { $trainerPath }
gameNotRunning =
    No running Proton game found.

    Start the game first (Steam, Heroic, Lutris or another Proton launcher), then launch the trainer.
multipleGamesRunning =
    Several Proton games are running:

    { $games }

    Set the Steam AppID on this trainer to choose one.
gameStarting =
    The game is still starting (its Proton prefix is not ready yet).

    Try again in a few seconds.
protonPathNotFound =
    Proton installation for this game not found.

    Prefix: { $prefix }
trainerExited =
    The trainer exited right away (exit code { $code }).

    { $output }

    Full log: { $logPath }
sourceCode = Source
basedOn = Based on LaLa
openLog = Open log
trainerLogs = Trainer logs
openFolder = Open folder
resetTrainerData = Reset trainer data
resetTrainerDataConfirm =
    For FLiNG trainers that report "previous trainer instance did not exit properly".

    Close the game and the trainer first. KaKa then deletes the FLiNGTrainer folder and empties the Temp folder of the game prefix:

    { $paths }

    Nothing else is changed.
resetTrainerDataDone = Trainer data reset ({ $count } items removed)
prefixInUse = The game or a trainer is still running in this prefix. Close them first, then try again.
cleanupOutsidePrefix = Not reset: the AppData folder of this prefix points outside the prefix.
running = Running
stop = Stop
trainerAlreadyRunning = { $name } is already running.
trainerStopped = Trainer stopped: { $name }
trainerStopFailed = { $name } did not exit yet. Close it from its own window.
useRunningGame = Use running game
chooseGame = Which game?
quit = Quit
about = About KaKa
