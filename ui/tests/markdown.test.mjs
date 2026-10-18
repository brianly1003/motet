import {test} from 'node:test';
import assert from 'node:assert/strict';
import {renderNotes, displayTranscript} from '../lib/md.ts';
test('notes cannot execute HTML or load remote resources',()=>{
 const html=renderNotes('<script>alert(1)</script>\n\n![secret](https://example.com/pixel)\n\n[Open](https://example.com)');
 assert.ok(!html.includes('<script>'));
 assert.ok(!html.includes('<img'));
 assert.ok(!html.includes('href='));
 assert.ok(!html.includes('https://example.com'));
});
test('notes retain structure and separate the transcript',()=>{
 const html=renderNotes('### Action items\n\n- Alex: ship Friday\n\n<details><summary>Transcript</summary>\nprivate transcript');
 assert.ok(html.includes('<h3>Action items</h3>'));
 assert.ok(html.includes('Alex: ship Friday'));
 assert.ok(!html.includes('private transcript'));
});
test('transcript shows You for the mic and leaves other speakers alone',()=>{
 const out=displayTranscript('[00:01] Them: Me: is a word\n[00:03] Me: Sure\n[00:05] Speaker 2: ok');
 assert.equal(out,'[00:01] Them: Me: is a word\n[00:03] You: Sure\n[00:05] Speaker 2: ok');
});
