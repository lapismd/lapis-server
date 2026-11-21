type PasskeySummary = {
  id: string;
  label: string;
  created_at: number;
  last_used_at: number | null;
};

type Challenge<Options> = { id: string; options: { publicKey: Options } };

export interface AccountSecurity {
  signIn(username: string): Promise<void>;
  passkeys(): Promise<PasskeySummary[]>;
  enroll(label: string): Promise<void>;
  rename(id: string, label: string): Promise<void>;
  remove(id: string): Promise<void>;
  verifyPassword(password: string): Promise<void>;
  verifyPasskey(): Promise<void>;
}

type Request = <T>(
  path: string,
  method?: string,
  value?: unknown,
) => Promise<T>;

export function createAccountSecurity(
  json: Request,
  acceptSession: (session: { csrf_token: string }) => void,
): AccountSecurity {
  const path = "/auth/passkeys";

  async function assertion(
    flow: "login" | "reauthenticate",
    input?: { username: string },
  ) {
    const challenge = await json<
      Challenge<PublicKeyCredentialRequestOptionsJSON>
    >(`${path}/${flow}/options`, "POST", input);
    const credential = await navigator.credentials.get({
      publicKey: PublicKeyCredential.parseRequestOptionsFromJSON(
        challenge.options.publicKey,
      ),
    });
    if (!(credential instanceof PublicKeyCredential))
      throw new Error("Passkey verification was canceled.");
    return {
      id: challenge.id,
      credential: credential.toJSON(),
    };
  }

  return {
    async signIn(username) {
      acceptSession(
        await json(
          `${path}/login/verify`,
          "POST",
          await assertion("login", { username }),
        ),
      );
    },
    passkeys: () => json(path),
    async enroll(label) {
      const challenge = await json<
        Challenge<PublicKeyCredentialCreationOptionsJSON>
      >(`${path}/register/options`, "POST", { label });
      const credential = await navigator.credentials.create({
        publicKey: PublicKeyCredential.parseCreationOptionsFromJSON(
          challenge.options.publicKey,
        ),
      });
      if (!(credential instanceof PublicKeyCredential))
        throw new Error("Passkey enrollment was canceled.");
      await json(`${path}/register/verify`, "POST", {
        id: challenge.id,
        credential: credential.toJSON(),
      });
    },
    rename: (id, label) =>
      json(`${path}/${encodeURIComponent(id)}`, "PATCH", { label }),
    remove: (id) => json(`${path}/${encodeURIComponent(id)}`, "DELETE"),
    verifyPassword: (password) =>
      json("/auth/reauthenticate", "POST", { password }),
    async verifyPasskey() {
      await json(
        `${path}/reauthenticate/verify`,
        "POST",
        await assertion("reauthenticate"),
      );
    },
  };
}

export function supportsPasskeys(): boolean {
  return (
    globalThis.isSecureContext &&
    typeof globalThis.PublicKeyCredential?.parseCreationOptionsFromJSON ===
      "function" &&
    typeof globalThis.PublicKeyCredential?.parseRequestOptionsFromJSON ===
      "function" &&
    typeof globalThis.PublicKeyCredential?.prototype.toJSON === "function"
  );
}

export function securityError(error: unknown): string {
  if (error instanceof DOMException && error.name === "NotAllowedError")
    return "Passkey verification was canceled or no matching passkey was available.";
  if (error instanceof DOMException && error.name === "InvalidStateError")
    return "This passkey is already registered.";
  return error instanceof Error ? error.message : "Passkey operation failed.";
}
