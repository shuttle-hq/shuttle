import {handleAuth, handleCallback, handleLogin} from "@auth0/nextjs-auth0";
import shuttle, {Error} from "../../../lib/shuttle";

async function afterCallback(req, res, session, state) {
  const shuttlified = session.user.sub.replace("|", "-");

  const user = await shuttle.get_user(shuttlified).catch((err) => {
    if ((err as Error).status === 404) {
      console.log(`user ${shuttlified} does not exist, creating`);
      return shuttle.create_user(shuttlified);
    } else {
      return Promise.reject(err);
    }
  });

  session.user.api_key = user.key;

  return session;
}

export default handleAuth({
  async callback(req, res) {
    try {
      await handleCallback(req, res, { afterCallback });
    } catch (error) {
      res.status(error.status || 500).end(error.message);
    }
  },
  async login(req, res) {
    try {
      await handleLogin(req, res, {
        authorizationParams: {
          connection: "github",
        },
      });
    } catch (error) {
      res.status(error.status || 400).end(error.message);
    }
  },
});
