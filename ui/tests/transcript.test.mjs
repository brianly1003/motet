import { test } from 'node:test';
import assert from 'node:assert/strict';
import { transcriptLines } from '../lib/transcript.ts';
test('marks retain their time and attach to the preceding utterance across gaps', () => {
 const lines = transcriptLines('[00:02] Me: Hello\n[01:10] Them: <script>\nplain text', [0, 25000, 75000]);
 assert.equal(lines[0].speaker, 'You');
 assert.deepEqual(lines[0].marks, [0,25000]);
 assert.deepEqual(lines[1].marks, [75000]);
 assert.equal(lines[1].text, '<script>');
 assert.equal(lines[2].text, 'plain text');
 assert.deepEqual(transcriptLines('', [1]), []);
});
