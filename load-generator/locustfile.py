import os
import random
import time
from pathlib import Path

from locust import HttpUser, TaskSet, between, constant, task
from locust.clients import HttpSession


class MyUser(HttpUser):
  wait_time = between(2,5)


  @task
  def test_first_upload_task(self):
    directory_path = "./test_files"

    all_items = os.listdir(directory_path)
    all_files = [f for f in all_items if os.path.isfile(os.path.join(directory_path, f))]  
    print("Directory contents: ", all_files)
    random_file = random.choice(all_files)
    with open(f"./test_files/{random_file}", "rb") as file:
      self.file_payload = file.read()
      self.file_name = Path(file.name).name

      print(f"File name : {self.file_name}")
    files = {
      'file': (self.file_name, self.file_payload)
    }
    with self.client.post(f"/file/upload?file_name={self.file_name}&bucket_name=sample",files=files, catch_response=True) as response:
      if response.status_code == 201:
        response.success()
    
      else:
        response.failure(f"File upload failed with status code {response.status_code}")

  @task
  def get_download_task(self):
    directory_path = "./test_files"

    all_items = os.listdir(directory_path)
    all_files = [f for f in all_items if os.path.isfile(os.path.join(directory_path, f))]  
    print("Directory contents: ", all_files)
    random_file = random.choice(all_files)
    with open(f"./test_files/{random_file}", "rb") as file:
      self.file_payload = file.read()
      self.file_name = Path(file.name).name

      print(f"File name : {self.file_name}")
    file_name = self.file_name
    with self.client.get(f"/file/download?key={file_name}&bucket_name=sample", catch_response=True) as response:
      if response.status_code == 200:
        response.success()
      else:
        response.failure(f"File download failed with status code {response.status_code}")
  

class TestCacheOnceSemantics(HttpUser):
  wait_time = constant(1)
  
  @task
  def target_api_endpoint(self):
    directory_path = "./test_files"
    all_items = os.listdir(directory_path)
    all_files = [f for f in all_items if os.path.isfile(os.path.join(directory_path, f))]  
    print("Directory contents: ", all_files)
    random_file = random.choice(all_files)
    with self.client.get(f"/file/download?file_name={random_file}&bucket_name=sample", catch_response=True) as response:
      if (response.status_code == 200):
        response.success()
      else:
        response.failure(f"Download failed with status code {response.status_code}")
    



  
