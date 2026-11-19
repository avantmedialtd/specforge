# Throwaway TLS fixtures

Test-only certificates for `openspec-app`'s trust tests (`outbound-https-trust`). Nothing here
protects anything: the keys are public on purpose, and no system trusts these certificates.

| File | What it is |
|---|---|
| `ca.pem` | A root CA, `CN=SpecForge Test Root CA`, standing in for a corporate TLS-inspection root. Its key was discarded. |
| `localhost.pem`, `localhost.key` | A server certificate for `localhost` and `127.0.0.1`, signed by `ca.pem`. |
| `self-signed.pem`, `self-signed.key` | A self-signed server certificate for the same names, in no root set. |

All are ECDSA P-256 with PKCS#8 keys, valid from 2026-01-01 to 2126-01-01.

To regenerate them (OpenSSL 3.4 or later, for `-not_before` / `-not_after`):

```sh
V="-not_before 20260101000000Z -not_after 21260101000000Z"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout ca.key -out ca.pem $V \
  -subj "/CN=SpecForge Test Root CA" -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,keyCertSign,cRLSign" -addext "subjectKeyIdentifier=hash"
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout localhost.key \
  -out localhost.csr -subj "/CN=localhost"
printf '%s\n' "basicConstraints=critical,CA:FALSE" "keyUsage=critical,digitalSignature" \
  "extendedKeyUsage=serverAuth" "subjectAltName=DNS:localhost,IP:127.0.0.1" \
  "authorityKeyIdentifier=keyid" "subjectKeyIdentifier=hash" > localhost.ext
openssl x509 -req -in localhost.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out localhost.pem $V \
  -extfile localhost.ext
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout self-signed.key \
  -out self-signed.pem $V -subj "/CN=localhost" -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=serverAuth" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
rm ca.key ca.srl localhost.csr localhost.ext
```
