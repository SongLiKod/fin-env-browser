import test from 'node:test';
import assert from 'node:assert/strict';
import { FinEnvClient, FinEnvError } from '../dist/index.js';

function mockFetch(handler) {
  return async (url, options) => {
    const result = await handler(url, options);
    return new Response(JSON.stringify(result.body), {
      status: result.status ?? 200,
      headers: { 'Content-Type': 'application/json' },
    });
  };
}

test('client sends API key and query parameters', async () => {
  let captured;
  const client = new FinEnvClient({
    apiKey: 'finenv_test',
    baseUrl: 'http://127.0.0.1:17321/api/v1/',
    fetch: mockFetch((url, options) => {
      captured = { url, options };
      return { body: [] };
    }),
  });
  const result = await client.listEnvironments({ search: '订单', runningOnly: true });
  assert.deepEqual(result, []);
  assert.match(captured.url, /\/api\/v1\/envs\?/);
  assert.match(captured.url, /runningOnly=true/);
  assert.equal(captured.options.headers.Authorization, 'Bearer finenv_test');
});

test('client serializes JSON bodies and confirmation headers', async () => {
  let captured;
  const client = new FinEnvClient({
    apiKey: 'finenv_test',
    fetch: mockFetch((url, options) => {
      captured = { url, options };
      return { body: { deleted: true } };
    }),
  });
  await client.deleteEnvironment('env-1');
  assert.equal(captured.options.method, 'DELETE');
  assert.equal(captured.options.headers['X-FinEnv-Confirm'], 'true');
  assert.match(captured.url, /envs\/env-1$/);
});

test('API errors become FinEnvError with code and status', async () => {
  const client = new FinEnvClient({
    apiKey: 'finenv_test',
    fetch: mockFetch(() => ({
      status: 403,
      body: { error: { code: 'forbidden', message: '无权限' } },
    })),
  });
  await assert.rejects(
    () => client.listEnvironments(),
    (error) => {
      assert.ok(error instanceof FinEnvError);
      assert.equal(error.status, 403);
      assert.equal(error.code, 'forbidden');
      return true;
    },
  );
});

test('fromEnv requires a key', () => {
  const previous = process.env.FINENV_API_KEY;
  delete process.env.FINENV_API_KEY;
  try {
    assert.throws(() => FinEnvClient.fromEnv(), FinEnvError);
  } finally {
    if (previous === undefined) delete process.env.FINENV_API_KEY;
    else process.env.FINENV_API_KEY = previous;
  }
});
