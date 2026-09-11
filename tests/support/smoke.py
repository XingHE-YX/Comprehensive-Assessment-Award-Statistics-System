"""Real HTTP acceptance, Python standard library only; never prints credentials."""
import html
import http.cookiejar
import io
import os
import re
import ssl
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zipfile


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class Client:
    def __init__(self, base):
        self.base = base
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()), NoRedirect(),
            urllib.request.HTTPSHandler(context=ssl.create_default_context(cafile=os.environ.get('SMOKE_CA_FILE')))
        )

    def request(self, path, fields=None, multipart=False, file=False, status=200):
        body = None
        headers = {}
        if fields is not None:
            if multipart:
                boundary = uuid.uuid4().hex
                parts = []
                for key, value in fields.items():
                    parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{key}"\r\n\r\n{value}\r\n'.encode())
                if file:
                    parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="attachments"; filename="smoke.pdf"\r\nContent-Type: application/pdf\r\n\r\n'.encode() + PROOF + b'\r\n')
                parts.append(f'--{boundary}--\r\n'.encode())
                body = b''.join(parts)
                headers['Content-Type'] = f'multipart/form-data; boundary={boundary}'
            else:
                body = urllib.parse.urlencode(fields).encode()
                headers['Content-Type'] = 'application/x-www-form-urlencoded'
        request = urllib.request.Request(self.base + path, data=body, headers=headers)
        try:
            response = self.opener.open(request, timeout=30)
        except urllib.error.HTTPError as response_error:
            response = response_error
        data = response.read()
        assert response.code == status, f'HTTP status mismatch at acceptance step: expected {status}, received {response.code}'
        return data, response.headers


def token(body):
    return html.unescape(re.search(r'name="csrf_token" value="([^"]+)"', body.decode()).group(1))


PROOF = b'%PDF-1.4\n% synthetic deployment acceptance proof\n%%EOF\n'


def main():
    base = os.environ['BASE_URL'].rstrip('/')
    assert urllib.parse.urlparse(base).scheme in ('http', 'https')
    username = os.environ['ADMIN_USERNAME']
    password = os.environ['ADMIN_PASSWORD']
    class_code = os.environ['CLASS_ACCESS_CODE']
    student, admin, anonymous = (Client(base) for _ in range(3))
    login = admin.request('/admin/login')[0]
    admin.request('/admin/login', dict(csrf_token=token(login), username=username, password=password), status=303)
    settings = admin.request('/admin/settings')[0]
    active_rows = [row for row in re.findall(r'<tr>.*?</tr>', settings.decode(), flags=re.S) if '当前学年' in row]
    assert len(active_rows) == 1, 'Prepare and activate an academic year before running the smoke test'
    year_id = re.search(r'/admin/years/([0-9]+)/students', active_rows[0]).group(1)
    identity = 'SMOKE-' + uuid.uuid4().hex[:12]
    admin.request('/admin/years/' + year_id + '/students', dict(csrf_token=token(settings), roster_text='姓名,学号\n部署验收样例,' + identity), multipart=True, status=303)
    assert student.request('/healthz')[0] == b'ok'
    page = student.request('/')[0]
    student.request('/access', {'csrf_token': token(page), 'access_code': class_code}, status=303)
    form = student.request('/submit')[0]
    date = os.environ.get('SMOKE_OBTAINED_DATE') or re.search(r'[0-9]{4}-[0-9]{2}-[0-9]{2}', form.decode()).group(0)
    fields = dict(csrf_token=token(form), student_name='部署验收样例', student_no=identity,
                  has_result='yes', result_name='部署验收成果', obtained_date=date,
                  category='academic_competition', competition_name='验收竞赛',
                  competition_type='A', level='国家', award_level='一等奖')
    _, headers = student.request('/submit', fields, multipart=True, file=True, status=303)
    receipt = student.request(headers['Location'])[0].decode()
    number = re.search(r'ZC[0-9]{4}-[0-9]{6}', receipt).group(0)
    edit_code = re.search(r'id="edit-code">([^<]+)', receipt).group(1)
    query = student.request('/query')[0]
    student.request('/query', dict(csrf_token=token(query), submission_no=number, edit_code=edit_code), status=303)
    detail = student.request('/query/' + number)[0]
    attachment = html.unescape(re.search(r'href="(/submissions/[^\"]+/attachments/[0-9]+)"', detail.decode()).group(1))
    assert student.request(attachment)[0] == PROOF
    anonymous.request(attachment, status=403)
    fields.update(csrf_token=token(detail), result_name='部署验收成果已修改')
    student.request('/query/' + number + '/update', fields, multipart=True, status=303)
    assert '部署验收成果已修改' in student.request('/query/' + number)[0].decode()
    dashboard = admin.request('/admin?' + urllib.parse.urlencode(dict(student_no=identity)))[0].decode()
    admin_path = re.search(r'href="(/admin/submissions/[0-9]+)"', dashboard).group(1)
    review = admin.request(admin_path)[0]
    admin.request(admin_path + '/review', dict(csrf_token=token(review), status='approved', review_note='部署验收', approved_score='2.50'), status=303)
    assert '已通过' in student.request('/query/' + number)[0].decode()
    workbook, headers = admin.request('/admin/export.xlsx?' + urllib.parse.urlencode(dict(student_no=identity)))
    assert headers['Content-Type'] == 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
    with zipfile.ZipFile(io.BytesIO(workbook)) as archive:
        assert 'xl/worksheets/sheet1.xml' in archive.namelist()
        assert 'xl/worksheets/sheet2.xml' in archive.namelist()
        assert '2.5' in archive.read('xl/worksheets/sheet2.xml').decode()
    # Optional private artifact enables a subsequent container-restart verification.
    if os.environ.get('SMOKE_STATE_FILE'):
        import json
        output = os.open(os.environ['SMOKE_STATE_FILE'], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(output, 'w') as handle:
            json.dump(dict(submission_no=number, edit_code=edit_code, attachment=attachment), handle)
    print('smoke: submission, protected attachment, query/update, review and XLSX passed')


if __name__ == '__main__':
    try:
        main()
    except Exception:
        # HTTP bodies and URLs may contain credentials; keep failures inspectable
        # through the application request id, never a traceback with request data.
        print('smoke: failed; check configuration and application request events', file=sys.stderr)
        sys.exit(1)
