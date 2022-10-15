import axios, {AxiosRequestConfig, AxiosResponse, HttpStatusCode, Method} from "axios";

export async function getApiKey(username: string): Promise<string> {
  const res = await fetch(
    `${process.env.SHUTTLE_API_BASE_URL}/users/${username}`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${process.env.SHUTTLE_ADMIN_SECRET}`,
      },
    }
  );

  if (res.ok) {
    const body = await res.json();
    return body["key"]
  } else {
    console.log(res);
    throw new Error("could not get api key.");
  }
}

export type User = {
  name: string
  key: string
  projects: string[]
}

export type Error = {
  status: HttpStatusCode
  error: string
}

export class Shuttle {
  private url(suffix: string): string {
    return `${process.env.SHUTTLE_API_BASE_URL}${suffix}`
  }

  private request(method: Method, path: string): Promise<Record<string, any>> {
    let req = {
      headers: {
        Authorization: `Bearer ${process.env.SHUTTLE_ADMIN_SECRET}`
      },
      method: method,
      url: this.url(path)
    };
    return axios.request(req).then((res) => {
      return res.data;
    }).catch((err) => {
      if (err.response) {
        return Promise.reject({
          status: err.response.status,
          ...err.response.data
        })
      } else {
        return Promise.reject(err);
      }
    })
  }

  async get_user(user: string): Promise<User> {
    return this.request("GET", `/users/${user}`).then((body) => {
      return body as User
    })
  }

  async create_user(user: string): Promise<User> {
    return this.request("POST", `/users/${user}`).then((body) => {
      return body as User
    })
  }
}

export default new Shuttle();

