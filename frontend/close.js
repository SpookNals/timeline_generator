'use strict';

// Finish an approved close directly, without emitting another close request.
function createCloseHandler({ windowHandle, hasUnsavedChanges, isBusy, confirmDiscard, report }) {
  let pending = false;
  return async event => {
    event.preventDefault();
    if (pending) return;
    if (isBusy()) {
      report('Please wait for the current operation to finish, then close the window.');
      return;
    }
    pending = true;
    try {
      if (hasUnsavedChanges() && !await confirmDiscard()) return;
      await windowHandle.destroy();
    } catch (error) {
      report(`Could not close the window: ${error}`, true);
    } finally {
      pending = false;
    }
  };
}

if (typeof module !== 'undefined') module.exports = { createCloseHandler };
