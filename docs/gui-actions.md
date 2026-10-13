# GUI event-handler action verbs

Dumped read-only from the running `Addon.exe` on 2026-09-20 with `python tools/research/annogui.py verbs`
(map at `g_pGuiManager+0x8C`, queue flags at `+0xA4`, listener vector at `+0xBC`). These are the
tag names allowed inside `<EVENTHANDLER>` in a `.dlg`/`.tpl`; the action id is `group << 20 | sub`,
and `GuiManager_DispatchAction` (`0x00D78C90`) sends the record to the listener object of that group
(`vtable+0x10`). `q=1` verbs are queued to the main thread instead of running inline. The listener
objects are the game's menu controllers, so the set that exists depends on which menus have been
constructed; this dump was taken in-game with the main menu already visited.

56 verbs in 65 listener groups.

## Listener groups

| group | vtable | class |
|---|---|---|
| 1 | `0x010E816C` | `CMainListener` |
| 3 | `0x010EF074` | `CSafetyCheckDialogListener` |
| 4 | `0x010F76C4` | `CWarehousemenuListener` |
| 5 | `0x010D7444` | `CObjectListMenuListener` |
| 6 | `0x010CE2CC` | `CInternetChatListener` |
| 7 | `0x0111BC34` | `CMainMenuListener` |
| 8 | `0x010DE278` | `CThirdMessageList` |
| 9 | `0x010E3344` | `CActionBarListener` |
| 10 | `0x010D7B1C` | `CObjectMenuPublicListener` |
| 11 | `0x010DB074` | `CStrategyMapWrapper` |
| 12 | `0x010EE2EC` | `CObjectMenuListener` |
| 13 | `0x010D79D0` | `CObjectmenuHousePortraitListener` |
| 14 | `0x010E8E08` | `CMenuButtonsListener` |
| 15 | `0x01106A4C` | `CMainMenuDownloadProfilesDlgListener` |
| 17 | `0x010F0BA4` | `CShipmenuListener` |
| 18 | `0x010E8A24` | `CManufactoryListener` |
| 19 | `0x010CD73C` | `CInfobarIslandInfoListener` |
| 20 | `0x010E9584` | `CMilitaryBuildingListener` |
| 21 | `0x010EE348` | `CObjectMenuListener` |
| 23 | `0x010C9494` | `CDiplomacyMenuListener` |
| 24 | `0x010E6678` | `CFullscreenDialogsMenuListener` |
| 25 | `0x010E7A10` | `CInfobarListener` |
| 26 | `0x010EA154` | `CNeutralBaseListener` |
| 27 | `0x010D4C48` | `CMiniMapWrapper` |
| 28 | `0x010E4D7C` | `CBuildmenuListener` |
| 29 | `0x010EAF58` | `CNeutralBuildingListener` |
| 30 | `0x010EE370` | `CObjectMenuListener` |
| 31 | `0x010E383C` | `CActiveTradeListener` |
| 32 | `0x010E64F4` | `CCSPWarehouseListener` |
| 33 | `0x010D7C38` | `CPopupApplyGraphicsSettings` |
| 34 | `0x010EF158` | `CSaveMessageListener` |
| 35 | `0x010E8D1C` | `CMarineBuildingListener` |
| 36 | `0x010EEF6C` | `CQuestLogListener` |
| 37 | `0x010D480C` | `CMarketplaceMenuListener` |
| 39 | `0x010D6B8C` | `CMultiselectionMenuListener` |
| 40 | `0x010F1718` | `CTransferPopupListener` |
| 41 | `0x010F7754` | `CWarehousemenuListener` |
| 42 | `0x010EC974` | `CNewsMenuListener` |
| 43 | `0x010C7AD0` | `CCultureMenuListener` |
| 45 | `0x0111C16C` | `CMainMenuProfileOnlineAccountDlgListener` |
| 46 | `0x010D7BBC` | `CObjectmenuResourceListener` |
| 47 | `0x010D8B38` | `CRightClickFrameListener` |
| 48 | `0x010DEA3C` | `CUploadDialogListener` |
| 49 | `0x010EE3B8` | `CObjectMenuListener` |
| 50 | `0x010E6610` | `CExitGameMenuListener` |
| 52 | `0x010D6FB0` | `CNetworkInfoListener` |
| 53 | `0x010E1A38` | `CActionButtonBookListener` |
| 54 | `0x010E3FEC` | `CBailiwickMenuListener` |
| 55 | `0x010EE394` | `CObjectMenuListener` |
| 56 | `0x01106740` | `CMainMenuChangePasswordDlgListener` |
| 58 | `0x010CCD84` | `CGlobalInfoDetailListener` |
| 59 | `0x010F7778` | `CWarehousemenuListener` |
| 60 | `0x010E7FBC` | `CLoadingScreenListener` |
| 61 | `0x010DB6F8` | `CSystemMessagesListener` |
| 62 | `0x010DE5F8` | `CToolTipListener` |
| 65 | `0x010E7C10` | `CIngameChatListener` |
| 68 | `0x0111C0A8` | `CMainMenuProfileChangeNameListener` |
| 74 | `0x011068B4` | `CMainMenuClosedBetaAccountDialogListener` |
| 77 | `0x010DB270` | `CStrategyMapWrapper` |
| 80 | `0x010F162C` | `CStatisticsMenuListener` |
| 82 | `0x0110833C` | `CMainMenuGameLobbyListener` |
| 83 | `0x010ECBF4` | `CObjectmenuEspionageListener` |
| 84 | `0x010D6FD0` | `CNetworkInfoListener` |
| 87 | `0x0111C404` | `CMainMenuSessionBrowserListener` |
| 89 | `0x0111BFEC` | `CMainMenuOnlineKeyActivationListener` |

## Verbs

| id | group | queued | verb | listener |
|---|---|---|---|---|
| `0x00100001` | 1 | 0 | `AUTOHIDE` | `CMainListener` |
| `0x00100002` | 1 | 0 | `CLEAR_EDITFIELD` | `CMainListener` |
| `0x00100003` | 1 | 0 | `CLEARSELECTION` | `CMainListener` |
| `0x00100004` | 1 | 0 | `EXECUTE_CONSOLE_COMMAND` | `CMainListener` |
| `0x00100005` | 1 | 0 | `FADEIN` | `CMainListener` |
| `0x00100006` | 1 | 0 | `FADEOUT` | `CMainListener` |
| `0x00100007` | 1 | 0 | `FETCH_KEYBOARD_FOCUS` | `CMainListener` |
| `0x00100008` | 1 | 0 | `FILLCOMBOBOX` | `CMainListener` |
| `0x00100009` | 1 | 0 | `FILLCOMBOBOX_WITH_COLORICONS` | `CMainListener` |
| `0x0010000A` | 1 | 0 | `FILLLISTBOX` | `CMainListener` |
| `0x0010000B` | 1 | 0 | `GAMESPEED_DECREASE` | `CMainListener` |
| `0x0010000C` | 1 | 0 | `GAMESPEED_INCREASE` | `CMainListener` |
| `0x0010000D` | 1 | 0 | `PAGELAYOUT_ADDPAGE` | `CMainListener` |
| `0x0010000E` | 1 | 0 | `PAGELAYOUT_NEXTPAGE` | `CMainListener` |
| `0x0010000F` | 1 | 0 | `PAGELAYOUT_PREVPAGE` | `CMainListener` |
| `0x00100010` | 1 | 0 | `PAGELAYOUT_SELECTPAGE` | `CMainListener` |
| `0x00100011` | 1 | 0 | `PAGELAYOUT_SETUP` | `CMainListener` |
| `0x00100012` | 1 | 0 | `PAGELAYOUT_UPDATEPAGETABS` | `CMainListener` |
| `0x00100013` | 1 | 0 | `POLLING_SET_ANSWER` | `CMainListener` |
| `0x00100014` | 1 | 0 | `REFRESH_SELECTION` | `CMainListener` |
| `0x00100015` | 1 | 0 | `RUNMODULE` | `CMainListener` |
| `0x00100016` | 1 | 0 | `SCALE` | `CMainListener` |
| `0x00100017` | 1 | 0 | `SELECT_CARD_IN_CARDLAYOUT` | `CMainListener` |
| `0x00100018` | 1 | 0 | `SELECT_NEXT_SHIP` | `CMainListener` |
| `0x00100019` | 1 | 0 | `SELECT_NEXT_WAREHOUSE` | `CMainListener` |
| `0x0010001A` | 1 | 0 | `SELECTCARD_FROM_THISSTATE` | `CMainListener` |
| `0x0010001B` | 1 | 0 | `SET_ALIGNMENT` | `CMainListener` |
| `0x0010001C` | 1 | 0 | `SET_CAMERA_TO_SELECTION` | `CMainListener` |
| `0x0010001D` | 1 | 0 | `SET_CENTRAL_INTERFACE_SHOWMODE` | `CMainListener` |
| `0x0010001E` | 1 | 0 | `SET_EXTENT` | `CMainListener` |
| `0x0010001F` | 1 | 0 | `SET_PAUSE` | `CMainListener` |
| `0x00100020` | 1 | 0 | `SET_TOGGLE_STATUS` | `CMainListener` |
| `0x00100021` | 1 | 0 | `SET_TOOLTIP_VISIBLE` | `CMainListener` |
| `0x00100022` | 1 | 0 | `SETACTIVE_FOR_ONEAVAILABLEGUID` | `CMainListener` |
| `0x00100023` | 1 | 0 | `SETACTIVESTATUS` | `CMainListener` |
| `0x00100024` | 1 | 0 | `SETEDITFIELDCONTENTS` | `CMainListener` |
| `0x00100025` | 1 | 0 | `SETEXTENT_FROM_RIGHT` | `CMainListener` |
| `0x00100026` | 1 | 0 | `SETICONTOSELECTEDGUID` | `CMainListener` |
| `0x00100027` | 1 | 0 | `SETICONTOSELECTEDPLAYER` | `CMainListener` |
| `0x00100028` | 1 | 0 | `SETICONTOSELECTEDPLAYEREMBLEM` | `CMainListener` |
| `0x00100029` | 1 | 0 | `SETKEYBOARDFOCUS` | `CMainListener` |
| `0x0010002A` | 1 | 0 | `SETPROGRESSBARVALUEFROMSLIDER` | `CMainListener` |
| `0x0010002B` | 1 | 0 | `SETVISIBLE_FROM_THISSTATE` | `CMainListener` |
| `0x0010002C` | 1 | 0 | `SMALLTALK` | `CMainListener` |
| `0x0010002D` | 1 | 0 | `SORT_LISTBOX` | `CMainListener` |
| `0x0010002E` | 1 | 0 | `TAKE_SCREENSHOT` | `CMainListener` |
| `0x0010002F` | 1 | 0 | `TOGGLE_MENU` | `CMainListener` |
| `0x00100030` | 1 | 1 | `FITSIZETOCHILDREN` | `CMainListener` |
| `0x00100031` | 1 | 1 | `RESTORE_DEFAULT_MENU_STATUS` | `CMainListener` |
| `0x00100032` | 1 | 1 | `SETVISIBLE` | `CMainListener` |
| `0x00100033` | 1 | 1 | `SKIP_INTRO` | `CMainListener` |
| `0x00100034` | 1 | 1 | `START_SEQUENCE` | `CMainListener` |
| `0x00200001` | 2 | 0 | `PLAYSOUNDGUID` | `-` |
| `0x00300001` | 3 | 0 | `CANCEL_DELAYED_ACTION` | `CSafetyCheckDialogListener` |
| `0x00300002` | 3 | 0 | `DO_DELAYED_ACTION` | `CSafetyCheckDialogListener` |
| `0x00300003` | 3 | 0 | `QUICK_LOAD` | `CSafetyCheckDialog` |
