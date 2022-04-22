resource "aws_s3_bucket" "logs" {
  bucket = "unveil-logs"
}

resource "aws_s3_bucket_policy" "allow_load_balancer_to_log" {
  bucket = aws_s3_bucket.logs.id
  policy = <<POLICY
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Principal": {
        "AWS": "arn:aws:iam::652711504416:root"
      },
      "Action": "s3:PutObject",
      "Resource": "${aws_s3_bucket.logs.arn}/*"
    },
    {
      "Effect": "Allow",
      "Principal": {
        "Service": "delivery.logs.amazonaws.com"
      },
      "Action": "s3:PutObject",
      "Resource": "${aws_s3_bucket.logs.arn}/*",
      "Condition": {
        "StringEquals": {
          "s3:x-amz-acl": "bucket-owner-full-control"
        }
      }
    },
    {
      "Effect": "Allow",
      "Principal": {
        "Service": "delivery.logs.amazonaws.com"
      },
      "Action": "s3:GetBucketAcl",
      "Resource": "arn:aws:s3:::${aws_s3_bucket.logs.bucket}"
    }
  ]
}
POLICY
}

resource "aws_efs_file_system" "user_data" {
  creation_token = "unveil-user-data"
}

resource "aws_efs_mount_target" "user_data_a" {
  file_system_id  = aws_efs_file_system.user_data.id
  subnet_id       = aws_subnet.backend_a.id
  security_groups = [aws_security_group.unreasonable.id]
}

resource "aws_efs_mount_target" "user_data_b" {
  file_system_id  = aws_efs_file_system.user_data.id
  subnet_id       = aws_subnet.backend_b.id
  security_groups = [aws_security_group.unreasonable.id]
}
