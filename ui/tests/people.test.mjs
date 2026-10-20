import {test} from 'node:test';
import assert from 'node:assert/strict';
import {matchPeople, alike, mentionWord} from '../lib/people.ts';
const names=['You','Kevin','Linh Tran'];
test('the picker offers a new name only when nothing matches',()=>{
 assert.deepEqual(matchPeople(names,''),names);
 assert.deepEqual(matchPeople(names,'Ke'),['Kevin']);
 assert.deepEqual(matchPeople(names,'tr'),['Linh Tran']);
 assert.deepEqual(matchPeople(names,'Anna Le'),['Anna Le']);
 assert.deepEqual(matchPeople(names,'  An:na[x] '),['An na x']);
});
test('a name fix is a correction, not a different person',()=>{
 assert.ok(alike('Kenvin','Kevin'));
 assert.ok(alike('Jen Lee Ang','Jianliang'));
 assert.ok(!alike('Kevin','Linh'));
 assert.ok(!alike('You','Kevin'));
});
test('My notes write a picked name as one word',()=>{
 assert.equal(mentionWord('Linh Tran'),'@Linh_Tran');
 assert.equal(mentionWord('Kevin'),'@Kevin');
});
