import time

from locust import HttpUser, TaskSet, between, constant, task
from locust.clients import HttpSession


class MyUser(HttpUser):
  wait_time = between(2,5)

  def on_start(self) -> None:
    with open("../test_files/5MB_clean.bin", "rb") as file:
      self.file_payload = file.read()
      self.file_name = file.name


  @task
  def test_first_upload_task(self):
    files = {
      'file': ('5MB_clean.bin', self.file_payload, 'image/png')
    }
    with self.client.post(f"/file/upload?file_name={time.time_ns()}&bucket_name=sample",files=files, catch_response=True) as response:
      if response.status_code == 201:
        response.success()
    
      else:
        response.failure(f"File failed with status code {response.status_code}")

  @task
  def get_download_task(self):
    with self.client.get("/file/download?key=1782748709135144000&bucket_name=sample", catch_response=True) as response:
      if response.status_code == 200:
        response.success()
      else:
        response.failure(f"File failed with status code {response.status_code}")



  
