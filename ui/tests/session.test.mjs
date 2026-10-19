import {test} from 'node:test';
import assert from 'node:assert/strict';
import {elapsedLabel, problemMessage} from '../lib/session.ts';
test('elapsed recording time does not wrap at an hour', () => {
  assert.equal(elapsedLabel(0), '00:00');
  assert.equal(elapsedLabel(3_661_999), '61:01');
});
test('permission failures give a recovery action and processing failures preserve files', () => {
  assert.match(problemMessage({type:'PermissionMissing',kind:'Microphone',side:'YourMicrophone'}), /Microphone.*System Settings/);
  assert.match(problemMessage({type:'NotesFailed',meeting_id:'1',reason:'model stopped'}), /model stopped.*Saved audio/);
});
