// Executable fixture only. The production Node host still accepts only cli.js run-driver.
const assert = require('node:assert/strict');
const childProcess = require('node:child_process');
const path = require('node:path');
const readline = require('node:readline');

// Any shell or external cleanup helper makes this fixture fail, including a hidden one.
const spawn = childProcess.spawn;
childProcess.spawn = (command, args, options) => {
  assert.equal(command, process.execPath);
  assert.ok(!options.shell);
  return spawn(command, args, { ...options, windowsHide: true });
};
childProcess.spawnSync = childProcess.execSync = childProcess.execFileSync = () => {
  throw new Error('Forced cleanup must not launch a helper process');
};
const { launchProcess } = require(path.join(process.cwd(), '.playwright/package/lib/coreBundle.js')).utils;

(async () => {
  const descendant = 'setInterval(() => {}, 1000)';
  const parent = `
    const child = require('node:child_process').spawn(process.execPath, ['-e', ${JSON.stringify(descendant)}],
      { windowsHide: true, stdio: 'ignore' });
    child.on('spawn', () => process.stdout.write(child.pid + '\\n'));
    setInterval(() => {}, 1000);
  `;
  let descendantPid;
  let ready;
  const descendantReady = new Promise(resolve => ready = resolve);
  const launched = await launchProcess({
    command: process.execPath, args: ['-e', parent], env: process.env,
    stdio: 'pipe', tempDirectories: [],
    log: message => {
      const match = /\[out\] (\d+)$/.exec(message);
      if (match) { descendantPid = Number(match[1]); ready(); }
    },
    onExit: () => {}, attemptToGracefullyClose: async () => { throw new Error('Force cleanup'); },
  });
  await descendantReady;
  process.stdout.write(`${launched.launchedProcess.pid} ${descendantPid}\n`);
  for await (const command of readline.createInterface({ input: process.stdin })) {
    if (command === 'force') {
      await launched.gracefullyClose();
      // A failed or suppressed kill must not pass. The orphan remains in the host's Job.
      assert.equal(launched.launchedProcess.signalCode, 'SIGKILL');
      process.stdout.write('cleanup-direct\n');
      process.exit(0);
    }
    assert.equal(command, 'health');
    setInterval(() => {}, 1000);
    return;
  }
})().catch(error => { console.error(error); process.exit(1); });
