const assert = require('node:assert/strict');
const { test } = require('node:test');
const { prepareRequest, submitRequest } = require('./registry-release.cjs');
const repository = { owner: 'Chiv2-Community', repo: 'UnchainedPlugin' };
const release = {
  draft: false, tag_name: 'v1.0.0-RC3',
  assets: [{ name: 'UnchainedPlugin.dll', state: 'uploaded', size: 123 }],
};
const source = data => ({ rest: { repos: { getReleaseByTag: async args => {
  assert.deepEqual(args, { ...repository, tag: 'v1.0.0-RC3' });
  return { data };
} } } });

test('request matches the DLL template and preserves the exact prerelease tag', async () => {
  const request = await prepareRequest(source(release), repository, release.tag_name);
  assert.equal(request.title, '[DLL RELEASE] UnchainedPlugin Version v1.0.0-RC3');
  const payload = JSON.parse(request.body.match(/```json\n([\s\S]*?)\n```/)[1]);
  assert.deepEqual(payload, {
    action: 'add-dll-release', repo_url: 'https://github.com/Chiv2-Community/UnchainedPlugin', release_tag: 'v1.0.0-RC3',
  });
});

test('invalid tags are rejected without contacting GitHub', async () => {
  for (const tag of ['main', 'v1', 'v1.0.0\nmalicious', 'v1.0.0"']) {
    await assert.rejects(prepareRequest({}, repository, tag), /semantic version/);
  }
});

test('unpublished, mismatched, missing, empty, or unfinished DLL releases are rejected', async () => {
  for (const data of [
    { ...release, draft: true }, { ...release, tag_name: 'v1.0.0' }, { ...release, assets: [] },
    { ...release, assets: [{ ...release.assets[0], size: 0 }] },
    { ...release, assets: [{ ...release.assets[0], state: 'new' }] },
  ]) await assert.rejects(prepareRequest(source(data), repository, release.tag_name));
});

test('creates a request in C2ModRegistry when none exists', async () => {
  const request = await prepareRequest(source(release), repository, release.tag_name);
  const github = {
    rest: { issues: { listForRepo: 'list', create: async args => {
      assert.deepEqual(args, { owner: 'Chiv2-Community', repo: 'C2ModRegistry', title: request.title, body: request.body });
      return { data: { html_url: 'https://github.com/Chiv2-Community/C2ModRegistry/issues/123' } };
    } } },
    paginate: async (method, args) => {
      assert.equal(method, 'list');
      assert.equal(args.state, 'all');
      return [];
    },
  };
  assert.equal((await submitRequest(github, request)).html_url, 'https://github.com/Chiv2-Community/C2ModRegistry/issues/123');
});

test('reruns reuse either open or closed requests, but never pull requests', async () => {
  const request = await prepareRequest(source(release), repository, release.tag_name);
  for (const state of ['open', 'closed']) {
    const existing = { state, body: request.body, html_url: 'existing' };
    const github = {
      rest: { issues: { listForRepo: 'list', create: async () => { throw new Error('Unexpected duplicate'); } } },
      paginate: async () => [{ body: request.body, pull_request: {}, html_url: 'not-an-issue' }, existing],
    };
    assert.equal(await submitRequest(github, request), existing);
  }
});

test('GitHub API failures propagate rather than reporting success', async () => {
  await assert.rejects(prepareRequest({ rest: { repos: { getReleaseByTag: async () => { throw new Error('release unavailable'); } } } }, repository, release.tag_name), /release unavailable/);
  await assert.rejects(submitRequest({ rest: { issues: { listForRepo: 'list' } }, paginate: async () => { throw new Error('permission denied'); } }, { marker: 'marker' }), /permission denied/);
});
