'use strict';

export const state = {
  _invoke: null,
  vaultEntries: [],
  vaultNotes: [],
  currentVaultPath: null,
  selectedEntryId: null,
  lockAttempts: 0,
  lockCooldown: false,
  clipSecs: 30,
  clipTimer: null,
  clipInterval: null,
  settingsDebounce: null,
  confirmCallback: null,
  passwordRevealTimers: new Map(),
  appUnlocked: false,
  passwordScores: {}
};
